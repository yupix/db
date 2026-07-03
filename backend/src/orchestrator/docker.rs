use bollard::Docker;
use bollard::container::{
    Config as ContainerConfig, CreateContainerOptions, InspectContainerOptions,
    RemoveContainerOptions, StartContainerOptions, StatsOptions, StopContainerOptions,
};
use bollard::image::CreateImageOptions;
use bollard::models::{HealthConfig, HealthStatusEnum, HostConfig, PortBinding};
use bollard::network::CreateNetworkOptions;
use futures_util::StreamExt;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::error::AppError;

const POSTGRES_IMAGE: &str = "postgres:16-alpine";
const PGBOUNCER_IMAGE: &str = "bitnami/pgbouncer:latest";

/// イメージがローカルになければ pull する。
async fn ensure_image(docker: &Docker, image: &str) -> Result<(), AppError> {
    if docker.inspect_image(image).await.is_ok() {
        return Ok(());
    }
    tracing::info!("Pulling image {image}...");
    let mut stream = docker.create_image(
        Some(CreateImageOptions {
            from_image: image,
            ..Default::default()
        }),
        None,
        None,
    );
    while let Some(item) = stream.next().await {
        item.map_err(|e| AppError::Internal(format!("Failed to pull {image}: {e}")))?;
    }
    tracing::info!("Pulled image {image}");
    Ok(())
}

/// A one-shot snapshot of a container's resource usage, derived from the
/// Docker stats API.
#[derive(Debug, Clone, Default)]
pub struct ContainerStats {
    pub cpu_pct: f64,
    pub mem_used_bytes: i64,
    pub mem_limit_bytes: i64,
    pub net_rx_bytes: i64,
    pub net_tx_bytes: i64,
    pub block_read_bytes: i64,
    pub block_write_bytes: i64,
}

/// Fetch a single resource snapshot for a running container. CPU percent is
/// computed from the delta between the current and previous cpu readings the
/// Docker daemon includes in a one-shot stats response.
pub async fn container_stats(
    docker: &Docker,
    container_id: &str,
) -> Result<ContainerStats, AppError> {
    // one_shot must be false: with one_shot=true the daemon returns immediately
    // and leaves precpu_stats zeroed, which would make the CPU delta (and thus
    // cpu_pct) always read ~0. false makes it do the two-read cycle server-side
    // so precpu_stats is populated. stream=false still yields a single frame.
    let options = StatsOptions {
        stream: false,
        one_shot: false,
    };

    let stats = docker
        .stats(container_id, Some(options))
        .next()
        .await
        .ok_or_else(|| AppError::Internal("No stats returned for container".into()))??;

    // CPU percentage: (container delta / system delta) * online cpus * 100.
    let cpu_delta = stats.cpu_stats.cpu_usage.total_usage as f64
        - stats.precpu_stats.cpu_usage.total_usage as f64;
    let system_delta = stats.cpu_stats.system_cpu_usage.unwrap_or(0) as f64
        - stats.precpu_stats.system_cpu_usage.unwrap_or(0) as f64;
    let online_cpus = stats.cpu_stats.online_cpus.unwrap_or_else(|| {
        stats
            .cpu_stats
            .cpu_usage
            .percpu_usage
            .as_ref()
            .map(|v| v.len() as u64)
            .unwrap_or(1)
    }) as f64;
    let cpu_pct = if system_delta > 0.0 && cpu_delta > 0.0 {
        (cpu_delta / system_delta) * online_cpus * 100.0
    } else {
        0.0
    };

    let mem_used = stats.memory_stats.usage.unwrap_or(0);
    // Subtract reclaimable page cache to match `docker stats` reporting. cgroup
    // v1 exposes this as `cache`; cgroup v2 (modern default) as `inactive_file`.
    // Without handling v2, memory would be over-reported on most hosts.
    let cache = stats
        .memory_stats
        .stats
        .as_ref()
        .map(|s| match s {
            bollard::container::MemoryStatsStats::V1(v1) => v1.cache,
            bollard::container::MemoryStatsStats::V2(v2) => v2.inactive_file,
        })
        .unwrap_or(0);
    let mem_used_bytes = mem_used.saturating_sub(cache) as i64;
    let mem_limit_bytes = stats.memory_stats.limit.unwrap_or(0) as i64;

    let (net_rx_bytes, net_tx_bytes) = stats
        .networks
        .as_ref()
        .map(|nets| {
            nets.values().fold((0u64, 0u64), |(rx, tx), n| {
                (rx + n.rx_bytes, tx + n.tx_bytes)
            })
        })
        .unwrap_or((0, 0));

    let (block_read_bytes, block_write_bytes) = stats
        .blkio_stats
        .io_service_bytes_recursive
        .as_ref()
        .map(|entries| {
            entries.iter().fold((0u64, 0u64), |(r, w), e| {
                match e.op.to_lowercase().as_str() {
                    "read" => (r + e.value, w),
                    "write" => (r, w + e.value),
                    _ => (r, w),
                }
            })
        })
        .unwrap_or((0, 0));

    Ok(ContainerStats {
        cpu_pct,
        mem_used_bytes,
        mem_limit_bytes,
        net_rx_bytes: net_rx_bytes as i64,
        net_tx_bytes: net_tx_bytes as i64,
        block_read_bytes: block_read_bytes as i64,
        block_write_bytes: block_write_bytes as i64,
    })
}

const HEALTH_CHECK_TIMEOUT: Duration = Duration::from_secs(90);
const HEALTH_CHECK_INTERVAL: Duration = Duration::from_secs(2);

// ---------------------------------------------------------------------------
// Docker Network
// ---------------------------------------------------------------------------

pub async fn create_docker_network(
    docker: &Docker,
    network_name: &str,
) -> Result<String, AppError> {
    let options = CreateNetworkOptions {
        name: network_name,
        check_duplicate: true,
        ..Default::default()
    };

    let result = docker.create_network(options).await?;
    let id = result.id;
    tracing::info!("Created Docker network: {} ({})", network_name, id);
    Ok(id)
}

pub async fn remove_docker_network(docker: &Docker, network_id: &str) -> Result<(), AppError> {
    docker.remove_network(network_id).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Postgres Container
// ---------------------------------------------------------------------------

pub async fn create_postgres_container(
    docker: &Docker,
    container_name: &str,
    db_name: &str,
    db_user: &str,
    db_password: &str,
    port: u16,
    network_id: Option<&str>,
) -> Result<String, AppError> {
    ensure_image(docker, POSTGRES_IMAGE).await?;
    let mut port_bindings = HashMap::new();
    port_bindings.insert(
        "5432/tcp".to_string(),
        Some(vec![PortBinding {
            host_ip: Some("0.0.0.0".to_string()),
            host_port: Some(port.to_string()),
        }]),
    );

    let pg_db = format!("POSTGRES_DB={}", db_name);
    let pg_user = format!("POSTGRES_USER={}", db_user);
    let pg_pass = format!("POSTGRES_PASSWORD={}", db_password);
    let env = vec![pg_db.as_str(), pg_user.as_str(), pg_pass.as_str()];

    let healthcheck = HealthConfig {
        test: Some(vec![
            "CMD-SHELL".to_string(),
            "pg_isready -U $POSTGRES_USER -d $POSTGRES_DB".to_string(),
        ]),
        interval: Some(5_000_000_000),
        timeout: Some(5_000_000_000),
        retries: Some(10),
        start_period: Some(10_000_000_000),
        ..Default::default()
    };

    let mut host_config = HostConfig {
        port_bindings: Some(port_bindings),
        ..Default::default()
    };

    if let Some(net_id) = network_id {
        host_config.network_mode = Some(net_id.to_string());
    }

    let config = ContainerConfig {
        image: Some(POSTGRES_IMAGE),
        env: Some(env),
        host_config: Some(host_config),
        healthcheck: Some(healthcheck),
        // Preload pg_stat_statements so per-query statistics are available
        // (Phase 7.1). The extension itself is created after the DB is healthy.
        cmd: Some(vec![
            "postgres",
            "-c",
            "shared_preload_libraries=pg_stat_statements",
        ]),
        ..Default::default()
    };

    let options = Some(CreateContainerOptions {
        name: container_name,
        platform: None,
    });

    let result = docker.create_container(options, config).await?;
    docker
        .start_container(&result.id, None::<StartContainerOptions<String>>)
        .await?;

    tracing::info!(
        "Created and started container: {} ({}), waiting for healthy...",
        container_name,
        result.id
    );

    wait_for_healthy(docker, &result.id).await?;

    // Best-effort: enable the pg_stat_statements extension. Non-fatal — a
    // failure here just means query stats won't be available for this project.
    if let Err(e) = enable_pg_stat_statements(&result.id, db_user, db_name, db_password).await {
        tracing::warn!(
            "Failed to enable pg_stat_statements for {}: {}",
            container_name,
            e
        );
    }

    tracing::info!("Container {} is healthy", container_name);
    Ok(result.id)
}

/// Upper bound on an in-container `psql` exec so a wedged container can't block
/// the caller (a startup task or a request handler) indefinitely.
const PSQL_EXEC_TIMEOUT: Duration = Duration::from_secs(10);

/// Run `CREATE EXTENSION IF NOT EXISTS pg_stat_statements` inside the container.
/// Idempotent; safe to call again to lazily recover from a transient failure at
/// provisioning time. Fails on containers without the required
/// `shared_preload_libraries` preload (i.e. created before Phase 7.1).
pub(crate) async fn enable_pg_stat_statements(
    container_id: &str,
    db_user: &str,
    db_name: &str,
    db_password: &str,
) -> Result<(), AppError> {
    let mut cmd = tokio::process::Command::new("docker");
    cmd.args([
        "exec",
        container_id,
        "psql",
        "-U",
        db_user,
        "-d",
        db_name,
        "-q",
        "-c",
        "CREATE EXTENSION IF NOT EXISTS pg_stat_statements",
    ])
    .env("PGPASSWORD", db_password);

    let output = tokio::time::timeout(PSQL_EXEC_TIMEOUT, cmd.output())
        .await
        .map_err(|_| AppError::Internal("psql exec timed out".into()))?
        .map_err(|e| AppError::Internal(format!("Failed to run psql: {}", e)))?;

    if !output.status.success() {
        return Err(AppError::Internal(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    Ok(())
}

/// Run a read-only SQL query inside the container and return CSV (header + rows).
/// Used for introspection queries such as `pg_stat_statements`.
///
/// # Safety
/// `sql` is interpolated verbatim into `COPY ({sql}) TO STDOUT` and runs as the
/// container superuser (COPY can reach `PROGRAM`/file writes). Callers MUST pass
/// only trusted, constant SQL — never user input. Kept `pub(crate)` for this
/// reason.
pub(crate) async fn psql_query_csv(
    container_id: &str,
    db_user: &str,
    db_name: &str,
    db_password: &str,
    sql: &str,
) -> Result<String, AppError> {
    let wrapped = format!(
        "COPY ({}) TO STDOUT WITH CSV HEADER",
        sql.trim().trim_end_matches(';')
    );

    let mut cmd = tokio::process::Command::new("docker");
    cmd.args([
        "exec",
        "-i",
        container_id,
        "psql",
        "-U",
        db_user,
        "-d",
        db_name,
        "-q",
        "-v",
        "ON_ERROR_STOP=1",
        "-c",
        &wrapped,
    ])
    .env("PGPASSWORD", db_password);

    let output = tokio::time::timeout(PSQL_EXEC_TIMEOUT, cmd.output())
        .await
        .map_err(|_| AppError::Internal("psql exec timed out".into()))?
        .map_err(|e| AppError::Internal(format!("Failed to run psql: {}", e)))?;

    if !output.status.success() {
        return Err(AppError::BadRequest(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

// ---------------------------------------------------------------------------
// PgBouncer Container
// ---------------------------------------------------------------------------

pub struct PgBouncerConfig {
    pub container_name: String,
    pub port: u16,
    pub backend_host: String,
    pub backend_port: u16,
    pub backend_db: String,
    pub backend_user: String,
    pub backend_password: String,
    pub pool_mode: String,
    pub max_client_conn: i32,
    pub default_pool_size: i32,
    pub network_id: Option<String>,
}

pub async fn create_pgbouncer_container(
    docker: &Docker,
    config: &PgBouncerConfig,
) -> Result<String, AppError> {
    ensure_image(docker, PGBOUNCER_IMAGE).await?;
    let mut port_bindings = HashMap::new();
    port_bindings.insert(
        "6432/tcp".to_string(),
        Some(vec![PortBinding {
            host_ip: Some("0.0.0.0".to_string()),
            host_port: Some(config.port.to_string()),
        }]),
    );

    let env_vars: Vec<String> = vec![
        "PGBOUNCER_LISTEN_PORT=6432".to_string(),
        format!("PGBOUNCER_BACKEND_HOST={}", config.backend_host),
        format!("PGBOUNCER_BACKEND_PORT={}", config.backend_port),
        format!("PGBOUNCER_BACKEND_DATABASE={}", config.backend_db),
        format!("PGBOUNCER_BACKEND_USER={}", config.backend_user),
        format!("PGBOUNCER_BACKEND_PASSWORD={}", config.backend_password),
        format!("PGBOUNCER_POOL_MODE={}", config.pool_mode),
        format!("PGBOUNCER_MAX_CLIENT_CONN={}", config.max_client_conn),
        format!("PGBOUNCER_DEFAULT_POOL_SIZE={}", config.default_pool_size),
        "PGBOUNCER_AUTH_TYPE=trust".to_string(),
    ];
    let env: Vec<&str> = env_vars.iter().map(|s| s.as_str()).collect();

    let mut host_config = HostConfig {
        port_bindings: Some(port_bindings),
        ..Default::default()
    };

    if let Some(net_id) = &config.network_id {
        host_config.network_mode = Some(net_id.clone());
    }

    let healthcheck = HealthConfig {
        test: Some(vec![
            "CMD-SHELL".to_string(),
            "pg_isready -h 127.0.0.1 -p 6432".to_string(),
        ]),
        interval: Some(5_000_000_000),
        timeout: Some(5_000_000_000),
        retries: Some(10),
        start_period: Some(5_000_000_000),
        ..Default::default()
    };

    let container_config = ContainerConfig {
        image: Some(PGBOUNCER_IMAGE),
        env: Some(env),
        host_config: Some(host_config),
        healthcheck: Some(healthcheck),
        ..Default::default()
    };

    let options = Some(CreateContainerOptions {
        name: &config.container_name,
        platform: None,
    });

    let result = docker.create_container(options, container_config).await?;
    docker
        .start_container(&result.id, None::<StartContainerOptions<String>>)
        .await?;

    tracing::info!(
        "Created and started PgBouncer container: {} ({}), waiting for healthy...",
        config.container_name,
        result.id
    );

    wait_for_healthy(docker, &result.id).await?;

    tracing::info!("PgBouncer container {} is healthy", config.container_name);
    Ok(result.id)
}

// ---------------------------------------------------------------------------
// Shared utilities
// ---------------------------------------------------------------------------

pub async fn wait_for_healthy(docker: &Docker, container_id: &str) -> Result<(), AppError> {
    let start = Instant::now();

    loop {
        if start.elapsed() > HEALTH_CHECK_TIMEOUT {
            // タイムアウト時に healthcheck ログを出力して原因を診断しやすくする
            if let Ok(inspect) = docker
                .inspect_container(container_id, None::<InspectContainerOptions>)
                .await
            {
                if let Some(logs) = inspect
                    .state
                    .as_ref()
                    .and_then(|s| s.health.as_ref())
                    .and_then(|h| h.log.as_ref())
                {
                    for entry in logs.iter().rev().take(3) {
                        tracing::error!(
                            "  healthcheck: exit={:?} output={:?}",
                            entry.exit_code,
                            entry.output
                        );
                    }
                }
            }
            return Err(AppError::Internal(format!(
                "Container {} did not become healthy within {:?}",
                container_id, HEALTH_CHECK_TIMEOUT
            )));
        }

        let inspect = docker
            .inspect_container(container_id, None::<InspectContainerOptions>)
            .await?;

        let health = inspect.state.as_ref().and_then(|s| s.health.as_ref());

        match health {
            Some(h) if h.status == Some(HealthStatusEnum::HEALTHY) => {
                return Ok(());
            }
            Some(h) if h.status == Some(HealthStatusEnum::UNHEALTHY) => {
                return Err(AppError::Internal(format!(
                    "Container {} is unhealthy",
                    container_id
                )));
            }
            _ => {
                tokio::time::sleep(HEALTH_CHECK_INTERVAL).await;
            }
        }
    }
}

pub async fn start_container(docker: &Docker, container_id: &str) -> Result<(), AppError> {
    docker
        .start_container(container_id, None::<StartContainerOptions<String>>)
        .await?;

    tracing::info!(
        "Starting container {}, waiting for healthy...",
        container_id
    );
    wait_for_healthy(docker, container_id).await?;

    Ok(())
}

pub async fn stop_container(docker: &Docker, container_id: &str) -> Result<(), AppError> {
    docker
        .stop_container(container_id, None::<StopContainerOptions>)
        .await?;
    Ok(())
}

pub async fn remove_container(docker: &Docker, container_id: &str) -> Result<(), AppError> {
    let options = Some(RemoveContainerOptions {
        force: true,
        ..Default::default()
    });
    docker.remove_container(container_id, options).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Branch Container (pg_basebackup)
// ---------------------------------------------------------------------------

pub struct BranchConfig {
    pub container_name: String,
    pub db_name: String,
    pub db_user: String,
    pub db_password: String,
    pub port: u16,
    pub network_id: Option<String>,
}

/// Start a blank Postgres container with the given config, wait until healthy,
/// and return the container ID. No data is copied — callers handle that.
async fn spawn_blank_postgres_container(
    docker: &Docker,
    config: &BranchConfig,
) -> Result<String, AppError> {
    ensure_image(docker, POSTGRES_IMAGE).await?;
    let mut port_bindings = HashMap::new();
    port_bindings.insert(
        "5432/tcp".to_string(),
        Some(vec![PortBinding {
            host_ip: Some("0.0.0.0".to_string()),
            host_port: Some(config.port.to_string()),
        }]),
    );

    let pg_db = format!("POSTGRES_DB={}", config.db_name);
    let pg_user = format!("POSTGRES_USER={}", config.db_user);
    let pg_pass = format!("POSTGRES_PASSWORD={}", config.db_password);
    let env = vec![pg_db.as_str(), pg_user.as_str(), pg_pass.as_str()];

    let healthcheck = HealthConfig {
        test: Some(vec![
            "CMD-SHELL".to_string(),
            "pg_isready -U $POSTGRES_USER -d $POSTGRES_DB".to_string(),
        ]),
        interval: Some(5_000_000_000),
        timeout: Some(5_000_000_000),
        retries: Some(10),
        start_period: Some(10_000_000_000),
        ..Default::default()
    };

    let mut host_config = HostConfig {
        port_bindings: Some(port_bindings),
        ..Default::default()
    };

    if let Some(net_id) = &config.network_id {
        host_config.network_mode = Some(net_id.clone());
    }

    let container_config = ContainerConfig {
        image: Some(POSTGRES_IMAGE),
        env: Some(env),
        host_config: Some(host_config),
        healthcheck: Some(healthcheck),
        ..Default::default()
    };

    let options = Some(CreateContainerOptions {
        name: &config.container_name,
        platform: None,
    });

    let result = docker.create_container(options, container_config).await?;
    let container_id = result.id;

    docker
        .start_container(&container_id, None::<StartContainerOptions<String>>)
        .await?;

    tracing::info!(
        "Branch container {} started, waiting for healthy...",
        config.container_name
    );

    wait_for_healthy(docker, &container_id).await?;

    Ok(container_id)
}

/// Restore a `pg_dump -Fc` backup archive into a new branch container.
pub async fn create_branch_from_backup(
    docker: &Docker,
    config: &BranchConfig,
    backup_path: &std::path::PathBuf,
) -> Result<String, AppError> {
    let container_id = spawn_blank_postgres_container(docker, config).await?;

    let result = crate::orchestrator::backup::restore_from_file(
        &container_id,
        &config.db_user,
        &config.db_name,
        &config.db_password,
        backup_path,
    )
    .await;

    if let Err(e) = result {
        let _ = remove_container(docker, &container_id).await;
        return Err(e);
    }

    tracing::info!(
        "Branch container {} restored from backup successfully",
        config.container_name
    );

    Ok(container_id)
}

pub async fn create_branch_container(
    docker: &Docker,
    config: &BranchConfig,
    source_container_id: &str,
    source_db_name: &str,
    source_db_user: &str,
) -> Result<String, AppError> {
    let branch_container_id = spawn_blank_postgres_container(docker, config).await?;

    // Copy data: pg_dump from source -> collect -> psql to branch (cross-platform)
    let pg_dump_output = tokio::process::Command::new("docker")
        .args([
            "exec",
            source_container_id,
            "pg_dump",
            "-U",
            source_db_user,
            "-d",
            source_db_name,
            "--no-owner",
            "--no-privileges",
        ])
        .output()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to run pg_dump: {}", e)))?;

    if !pg_dump_output.status.success() {
        let stderr = String::from_utf8_lossy(&pg_dump_output.stderr);
        let _ = remove_container(docker, &branch_container_id).await;
        return Err(AppError::Internal(format!("pg_dump failed: {}", stderr)));
    }

    // Feed dump output to psql in branch container
    let mut psql_child = tokio::process::Command::new("docker")
        .args([
            "exec",
            "-i",
            &branch_container_id,
            "psql",
            "-U",
            &config.db_user,
            "-d",
            &config.db_name,
            "-q",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| AppError::Internal(format!("Failed to start psql: {}", e)))?;

    {
        use tokio::io::AsyncWriteExt;
        if let Some(mut stdin) = psql_child.stdin.take() {
            stdin
                .write_all(&pg_dump_output.stdout)
                .await
                .map_err(|e| AppError::Internal(format!("Failed to write to psql: {}", e)))?;
        }
    }

    let psql_result = psql_child
        .wait_with_output()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to wait for psql: {}", e)))?;

    if !psql_result.status.success() {
        let stderr = String::from_utf8_lossy(&psql_result.stderr);
        let _ = remove_container(docker, &branch_container_id).await;
        return Err(AppError::Internal(format!("psql failed: {}", stderr)));
    }

    tracing::info!(
        "Branch container {} created and data copied successfully",
        config.container_name
    );

    Ok(branch_container_id)
}

pub async fn reset_branch_container(
    _docker: &Docker,
    branch_container_id: &str,
    source_container_id: &str,
    source_db_name: &str,
    source_db_user: &str,
    branch_db_name: &str,
    branch_db_user: &str,
) -> Result<(), AppError> {
    // Drop and recreate the database (cross-platform docker CLI)
    let drop_output = tokio::process::Command::new("docker")
        .args([
            "exec",
            branch_container_id,
            "psql",
            "-U",
            branch_db_user,
            "-d",
            "postgres",
            "-c",
            &format!("DROP DATABASE IF EXISTS {}", branch_db_name),
        ])
        .output()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to drop database: {}", e)))?;

    if !drop_output.status.success() {
        return Err(AppError::Internal(format!(
            "Failed to drop database: {}",
            String::from_utf8_lossy(&drop_output.stderr)
        )));
    }

    let create_output = tokio::process::Command::new("docker")
        .args([
            "exec",
            branch_container_id,
            "psql",
            "-U",
            branch_db_user,
            "-d",
            "postgres",
            "-c",
            &format!("CREATE DATABASE {}", branch_db_name),
        ])
        .output()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to create database: {}", e)))?;

    if !create_output.status.success() {
        return Err(AppError::Internal(format!(
            "Failed to create database: {}",
            String::from_utf8_lossy(&create_output.stderr)
        )));
    }

    // Copy data: pg_dump from source -> collect -> psql to branch
    let pg_dump_output = tokio::process::Command::new("docker")
        .args([
            "exec",
            source_container_id,
            "pg_dump",
            "-U",
            source_db_user,
            "-d",
            source_db_name,
            "--no-owner",
            "--no-privileges",
        ])
        .output()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to run pg_dump: {}", e)))?;

    if !pg_dump_output.status.success() {
        return Err(AppError::Internal(format!(
            "pg_dump failed: {}",
            String::from_utf8_lossy(&pg_dump_output.stderr)
        )));
    }

    let mut psql_child = tokio::process::Command::new("docker")
        .args([
            "exec",
            "-i",
            branch_container_id,
            "psql",
            "-U",
            branch_db_user,
            "-d",
            branch_db_name,
            "-q",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| AppError::Internal(format!("Failed to start psql: {}", e)))?;

    {
        use tokio::io::AsyncWriteExt;
        if let Some(mut stdin) = psql_child.stdin.take() {
            stdin
                .write_all(&pg_dump_output.stdout)
                .await
                .map_err(|e| AppError::Internal(format!("Failed to write to psql: {}", e)))?;
        }
    }

    let psql_result = psql_child
        .wait_with_output()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to wait for psql: {}", e)))?;

    if !psql_result.status.success() {
        return Err(AppError::Internal(format!(
            "psql failed: {}",
            String::from_utf8_lossy(&psql_result.stderr)
        )));
    }

    Ok(())
}
