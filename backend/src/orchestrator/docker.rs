use bollard::Docker;
use bollard::container::{
    Config as ContainerConfig, CreateContainerOptions, InspectContainerOptions,
    RemoveContainerOptions, StartContainerOptions, StopContainerOptions,
};
use bollard::models::{HealthConfig, HealthStatusEnum, HostConfig, PortBinding};
use bollard::network::CreateNetworkOptions;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::error::AppError;

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
        image: Some("postgres:16-alpine"),
        env: Some(env),
        host_config: Some(host_config),
        healthcheck: Some(healthcheck),
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

    tracing::info!("Container {} is healthy", container_name);
    Ok(result.id)
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
        image: Some("bitnami/pgbouncer:1.23"),
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

pub async fn create_branch_container(
    docker: &Docker,
    config: &BranchConfig,
    source_container_id: &str,
    source_db_name: &str,
    source_db_user: &str,
) -> Result<String, AppError> {
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
        image: Some("postgres:16-alpine"),
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
    let branch_container_id = result.id.clone();

    docker
        .start_container(&branch_container_id, None::<StartContainerOptions<String>>)
        .await?;

    tracing::info!(
        "Branch container {} created, waiting for healthy...",
        config.container_name
    );

    wait_for_healthy(docker, &branch_container_id).await?;

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
