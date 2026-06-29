use bollard::Docker;
use bollard::container::{
    Config as ContainerConfig, CreateContainerOptions, InspectContainerOptions,
    RemoveContainerOptions, StartContainerOptions, StopContainerOptions,
};
use bollard::models::{HealthConfig, HealthStatusEnum, HostConfig, PortBinding};
use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::error::AppError;

const HEALTH_CHECK_TIMEOUT: Duration = Duration::from_secs(90);
const HEALTH_CHECK_INTERVAL: Duration = Duration::from_secs(2);

pub async fn create_postgres_container(
    docker: &Docker,
    container_name: &str,
    db_name: &str,
    db_user: &str,
    db_password: &str,
    port: u16,
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

    let host_config = HostConfig {
        port_bindings: Some(port_bindings),
        ..Default::default()
    };

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
