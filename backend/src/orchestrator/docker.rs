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

    let container_config = ContainerConfig {
        image: Some("bitnami/pgbouncer:1.23"),
        env: Some(env),
        host_config: Some(host_config),
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
        "Created and started PgBouncer container: {} ({})",
        config.container_name,
        result.id
    );

    tokio::time::sleep(Duration::from_secs(3)).await;

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
