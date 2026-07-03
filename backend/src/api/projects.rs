use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use validator::Validate;

use crate::auth::jwt::AuthUser;
use crate::db::access::{Access, fetch_project_for};
use crate::db::models::{Project, ProjectEnvironment};
use crate::error::AppError;
use crate::orchestrator::docker;
use crate::state::AppState;
use crate::util::{random_string, slugify};

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_projects).post(create_project))
        .route(
            "/{id}",
            get(get_project)
                .delete(delete_project)
                .patch(update_project),
        )
        .route("/{id}/start", post(start_project))
        .route("/{id}/stop", post(stop_project))
        .route("/{id}/metrics", get(super::metrics::get_metrics))
        .route("/{id}/query-stats", get(super::metrics::get_query_stats))
        .route(
            "/{id}/alerts",
            get(super::metrics::list_alerts).post(super::metrics::create_alert),
        )
        .route(
            "/{id}/alerts/{alert_id}",
            axum::routing::patch(super::metrics::update_alert).delete(super::metrics::delete_alert),
        )
        .route(
            "/{id}/backups",
            get(super::backups::list_backups).post(super::backups::create_backup),
        )
        .route(
            "/{id}/backups/{backup_id}",
            axum::routing::delete(super::backups::delete_backup),
        )
        .route(
            "/{id}/backups/{backup_id}/restore",
            post(super::backups::restore_backup),
        )
        .route(
            "/{id}/backups/{backup_id}/restore-as-branch",
            post(super::branches::restore_as_branch),
        )
        .route(
            "/{id}/backup-policy",
            get(super::backups::get_backup_policy).patch(super::backups::update_backup_policy),
        )
        .route(
            "/{id}/pool",
            get(get_pool_settings).patch(update_pool_settings),
        )
        .route(
            "/{id}/environments",
            get(list_environments).post(create_environment),
        )
        .route(
            "/{id}/environments/{env_id}",
            get(get_environment).delete(delete_environment),
        )
        .route(
            "/{id}/branches",
            get(super::branches::list_branches).post(super::branches::create_branch),
        )
        .route(
            "/{id}/branches/{branch_id}",
            get(super::branches::get_branch)
                .delete(super::branches::delete_branch)
                .patch(super::branches::rename_branch),
        )
        .route(
            "/{id}/branches/{branch_id}/reset",
            post(super::branches::reset_branch),
        )
}

#[derive(Serialize)]
struct ProjectResponse {
    id: String,
    name: String,
    slug: String,
    status: String,
    port: i32,
    pgbouncer_port: Option<i32>,
    db_name: String,
    db_user: String,
    connection_string: String,
    pooled_connection_string: Option<String>,
    pool_mode: String,
    max_client_conn: i32,
    default_pool_size: i32,
    created_at: String,
}

impl ProjectResponse {
    fn from(project: &Project, host: &str) -> Self {
        let connection_string = format!(
            "postgres://{}:{}@{}:{}/{}",
            project.db_user, project.db_password, host, project.port, project.db_name
        );
        let pooled_connection_string = project.pgbouncer_port.map(|p| {
            format!(
                "postgres://{}:{}@{}:{}/{}",
                project.db_user, project.db_password, host, p, project.db_name
            )
        });

        Self {
            id: project.id.to_string(),
            name: project.name.clone(),
            slug: project.slug.clone(),
            status: project.status.clone(),
            port: project.port,
            pgbouncer_port: project.pgbouncer_port,
            db_name: project.db_name.clone(),
            db_user: project.db_user.clone(),
            connection_string,
            pooled_connection_string,
            pool_mode: project.pool_mode.clone(),
            max_client_conn: project.max_client_conn,
            default_pool_size: project.default_pool_size,
            created_at: project.created_at.to_rfc3339(),
        }
    }
}

async fn list_projects(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
) -> Result<Json<Vec<ProjectResponse>>, AppError> {
    // Includes projects owned directly, plus any assigned to a team the user
    // belongs to, plus projects assigned to a team in an org the user owns.
    let projects = sqlx::query_as::<_, Project>(
        "SELECT DISTINCT p.* FROM projects p
         LEFT JOIN project_teams pt ON pt.project_id = p.id
         LEFT JOIN team_members tm ON tm.team_id = pt.team_id AND tm.user_id = $1
         LEFT JOIN teams t ON t.id = pt.team_id
         LEFT JOIN organizations o ON o.id = t.org_id AND o.owner_id = $1
         WHERE p.status != 'deleted'
           AND (p.user_id = $1 OR tm.user_id = $1 OR o.owner_id = $1)
         ORDER BY p.created_at DESC",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(
        projects
            .iter()
            .map(|p| ProjectResponse::from(p, &state.config.public_host))
            .collect(),
    ))
}

#[derive(Deserialize, Validate)]
struct CreateProjectRequest {
    #[validate(length(min = 1, max = 100))]
    name: String,
    pool_mode: Option<String>,
    max_client_conn: Option<i32>,
    default_pool_size: Option<i32>,
}

async fn create_project(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
    Json(req): Json<CreateProjectRequest>,
) -> Result<Json<ProjectResponse>, AppError> {
    req.validate()
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    let slug = {
        let s = slugify(&req.name);
        if s.is_empty() {
            format!(
                "proj-{}",
                Uuid::new_v4()
                    .to_string()
                    .replace('-', "")
                    .chars()
                    .take(8)
                    .collect::<String>()
            )
        } else {
            s
        }
    };

    let existing_slug = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM projects WHERE slug = $1 AND status != 'deleted'",
    )
    .bind(&slug)
    .fetch_one(&state.db)
    .await?;

    if existing_slug > 0 {
        return Err(AppError::Conflict("Project name already exists".into()));
    }

    let db_name = format!("db_{}", &slug.replace('-', "_"));
    let db_user = format!(
        "user_{}",
        Uuid::new_v4()
            .to_string()
            .replace('-', "")
            .chars()
            .take(8)
            .collect::<String>()
    );
    let db_password = random_string(24);
    let container_name = format!(
        "userpg-{}",
        Uuid::new_v4()
            .to_string()
            .replace('-', "")
            .chars()
            .take(12)
            .collect::<String>()
    );
    let pgbouncer_name = format!(
        "pgbouncer-{}",
        Uuid::new_v4()
            .to_string()
            .replace('-', "")
            .chars()
            .take(12)
            .collect::<String>()
    );
    let network_name = format!(
        "net-{}",
        Uuid::new_v4()
            .to_string()
            .replace('-', "")
            .chars()
            .take(12)
            .collect::<String>()
    );

    let port = find_available_port(&state, &[]).await?;
    let pgbouncer_port = find_available_port(&state, &[port]).await?;

    let pool_mode = req.pool_mode.unwrap_or_else(|| "transaction".to_string());
    if !matches!(pool_mode.as_str(), "session" | "transaction" | "statement") {
        return Err(AppError::BadRequest(
            "pool_mode must be one of: session, transaction, statement".into(),
        ));
    }
    let max_client_conn = req.max_client_conn.unwrap_or(100);
    if !(1..=10000).contains(&max_client_conn) {
        return Err(AppError::BadRequest(
            "max_client_conn must be between 1 and 10000".into(),
        ));
    }
    let default_pool_size = req.default_pool_size.unwrap_or(20);
    if !(1..=1000).contains(&default_pool_size) {
        return Err(AppError::BadRequest(
            "default_pool_size must be between 1 and 1000".into(),
        ));
    }

    let project = sqlx::query_as::<_, Project>(
        "INSERT INTO projects (user_id, name, slug, container_name, db_name, db_user, db_password, port, status, pgbouncer_port, pool_mode, max_client_conn, default_pool_size)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'creating', $9, $10, $11, $12)
         RETURNING *",
    )
    .bind(user_id)
    .bind(&req.name)
    .bind(&slug)
    .bind(&container_name)
    .bind(&db_name)
    .bind(&db_user)
    .bind(&db_password)
    .bind(port)
    .bind(pgbouncer_port)
    .bind(&pool_mode)
    .bind(max_client_conn)
    .bind(default_pool_size)
    .fetch_one(&state.db)
    .await?;

    let state_clone = state.clone();
    let project_id = project.id;
    let cname = container_name.clone();
    let pgbouncer_cname = pgbouncer_name.clone();
    let net_name = network_name.clone();
    let dbn = db_name.clone();
    let dbu = db_user.clone();
    let dbp = db_password.clone();
    let pm = pool_mode.clone();
    let mcc = max_client_conn;
    let dps = default_pool_size;

    tokio::spawn(async move {
        let network_result = docker::create_docker_network(&state_clone.docker, &net_name).await;

        let network_id = match network_result {
            Ok(id) => {
                let _ = sqlx::query(
                    "UPDATE projects SET docker_network_id = $1, updated_at = now() WHERE id = $2",
                )
                .bind(&id)
                .bind(project_id)
                .execute(&state_clone.db)
                .await;
                id
            }
            Err(e) => {
                tracing::error!("Failed to create Docker network: {}", e);
                let _ = sqlx::query(
                    "UPDATE projects SET status = 'error', updated_at = now() WHERE id = $1",
                )
                .bind(project_id)
                .execute(&state_clone.db)
                .await;
                return;
            }
        };

        let pg_result = docker::create_postgres_container(
            &state_clone.docker,
            &cname,
            &dbn,
            &dbu,
            &dbp,
            port as u16,
            Some(&network_id),
        )
        .await;

        let _pg_container_id = match pg_result {
            Ok(id) => {
                let _ = sqlx::query(
                    "UPDATE projects SET container_id = $1, updated_at = now() WHERE id = $2",
                )
                .bind(&id)
                .bind(project_id)
                .execute(&state_clone.db)
                .await;
                id
            }
            Err(e) => {
                tracing::error!("Failed to create Postgres container: {}", e);
                let _ = sqlx::query(
                    "UPDATE projects SET status = 'error', updated_at = now() WHERE id = $1",
                )
                .bind(project_id)
                .execute(&state_clone.db)
                .await;
                return;
            }
        };

        let pgbouncer_config = docker::PgBouncerConfig {
            container_name: pgbouncer_cname.clone(),
            port: pgbouncer_port as u16,
            backend_host: cname.clone(),
            backend_port: 5432,
            backend_db: dbn.clone(),
            backend_user: dbu.clone(),
            backend_password: dbp.clone(),
            pool_mode: pm.clone(),
            max_client_conn: mcc,
            default_pool_size: dps,
            network_id: Some(network_id.clone()),
        };

        let pgb_result =
            docker::create_pgbouncer_container(&state_clone.docker, &pgbouncer_config).await;

        match pgb_result {
            Ok(pgb_id) => {
                let _ = sqlx::query(
                    "UPDATE projects SET pgbouncer_container_id = $1, status = 'running', updated_at = now() WHERE id = $2",
                )
                .bind(&pgb_id)
                .bind(project_id)
                .execute(&state_clone.db)
                .await;
                tracing::info!("Project {} fully provisioned (PG + PgBouncer)", project_id);
            }
            Err(e) => {
                tracing::error!("Failed to create PgBouncer container: {}", e);
                // Cleanup: remove PG container and network
                if let Ok(pg_cid) = sqlx::query_scalar::<_, String>(
                    "SELECT container_id FROM projects WHERE id = $1",
                )
                .bind(project_id)
                .fetch_one(&state_clone.db)
                .await
                    && !pg_cid.is_empty()
                {
                    let _ = docker::remove_container(&state_clone.docker, &pg_cid).await;
                }
                let _ = docker::remove_docker_network(&state_clone.docker, &network_id).await;
                let _ = sqlx::query(
                    "UPDATE projects SET status = 'error', container_id = NULL, docker_network_id = NULL, updated_at = now() WHERE id = $1",
                )
                .bind(project_id)
                .execute(&state_clone.db)
                .await;
            }
        }
    });

    Ok(Json(ProjectResponse::from(&project, &state.config.public_host)))
}

async fn get_project(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<ProjectResponse>, AppError> {
    let project = fetch_project_for(&state.db, id, user_id, Access::Read).await?;

    Ok(Json(ProjectResponse::from(&project, &state.config.public_host)))
}

async fn delete_project(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let project = fetch_project_for(&state.db, id, user_id, Access::Manage).await?;

    if let Some(cid) = &project.container_id {
        let _ = docker::remove_container(&state.docker, cid).await;
    }
    if let Some(pgb_cid) = &project.pgbouncer_container_id {
        let _ = docker::remove_container(&state.docker, pgb_cid).await;
    }
    if let Some(net_id) = &project.docker_network_id {
        let _ = docker::remove_docker_network(&state.docker, net_id).await;
    }

    sqlx::query("DELETE FROM projects WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;

    Ok(Json(serde_json::json!({ "deleted": true })))
}

async fn start_project(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<ProjectResponse>, AppError> {
    let project = fetch_project_for(&state.db, id, user_id, Access::Manage).await?;

    if let Some(cid) = &project.container_id {
        docker::start_container(&state.docker, cid).await?;
    }
    if let Some(pgb_cid) = &project.pgbouncer_container_id {
        let _ = docker::start_container(&state.docker, pgb_cid).await;
    }

    sqlx::query("UPDATE projects SET status = 'running', updated_at = now() WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;

    let updated = sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE id = $1")
        .bind(id)
        .fetch_one(&state.db)
        .await?;

    Ok(Json(ProjectResponse::from(&updated, &state.config.public_host)))
}

async fn stop_project(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<ProjectResponse>, AppError> {
    let project = fetch_project_for(&state.db, id, user_id, Access::Manage).await?;

    if let Some(pgb_cid) = &project.pgbouncer_container_id {
        let _ = docker::stop_container(&state.docker, pgb_cid).await;
    }
    if let Some(cid) = &project.container_id {
        docker::stop_container(&state.docker, cid).await?;
    }

    sqlx::query("UPDATE projects SET status = 'stopped', updated_at = now() WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;

    let updated = sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE id = $1")
        .bind(id)
        .fetch_one(&state.db)
        .await?;

    Ok(Json(ProjectResponse::from(&updated, &state.config.public_host)))
}

#[derive(Deserialize)]
struct UpdateProjectRequest {
    name: Option<String>,
}

async fn update_project(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateProjectRequest>,
) -> Result<Json<ProjectResponse>, AppError> {
    fetch_project_for(&state.db, id, user_id, Access::Manage).await?;

    if let Some(name) = &req.name {
        sqlx::query("UPDATE projects SET name = $1, updated_at = now() WHERE id = $2")
            .bind(name)
            .bind(id)
            .execute(&state.db)
            .await?;
    }

    let updated = sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE id = $1")
        .bind(id)
        .fetch_one(&state.db)
        .await?;

    Ok(Json(ProjectResponse::from(&updated, &state.config.public_host)))
}

// ---------------------------------------------------------------------------
// Pool Settings API (3.3)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct PoolSettingsResponse {
    pool_mode: String,
    max_client_conn: i32,
    default_pool_size: i32,
    pgbouncer_port: Option<i32>,
}

async fn get_pool_settings(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<PoolSettingsResponse>, AppError> {
    let project = fetch_project_for(&state.db, id, user_id, Access::Read).await?;

    Ok(Json(PoolSettingsResponse {
        pool_mode: project.pool_mode,
        max_client_conn: project.max_client_conn,
        default_pool_size: project.default_pool_size,
        pgbouncer_port: project.pgbouncer_port,
    }))
}

#[derive(Deserialize, Validate)]
struct UpdatePoolSettingsRequest {
    pool_mode: Option<String>,
    max_client_conn: Option<i32>,
    default_pool_size: Option<i32>,
}

async fn update_pool_settings(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdatePoolSettingsRequest>,
) -> Result<Json<PoolSettingsResponse>, AppError> {
    fetch_project_for(&state.db, id, user_id, Access::Manage).await?;

    if let Some(mode) = &req.pool_mode {
        if !matches!(mode.as_str(), "session" | "transaction" | "statement") {
            return Err(AppError::BadRequest(
                "pool_mode must be one of: session, transaction, statement".into(),
            ));
        }
        sqlx::query("UPDATE projects SET pool_mode = $1, updated_at = now() WHERE id = $2")
            .bind(mode)
            .bind(id)
            .execute(&state.db)
            .await?;
    }

    if let Some(max_conn) = req.max_client_conn {
        if !(1..=10000).contains(&max_conn) {
            return Err(AppError::BadRequest(
                "max_client_conn must be between 1 and 10000".into(),
            ));
        }
        sqlx::query("UPDATE projects SET max_client_conn = $1, updated_at = now() WHERE id = $2")
            .bind(max_conn)
            .bind(id)
            .execute(&state.db)
            .await?;
    }

    if let Some(pool_size) = req.default_pool_size {
        if !(1..=1000).contains(&pool_size) {
            return Err(AppError::BadRequest(
                "default_pool_size must be between 1 and 1000".into(),
            ));
        }
        sqlx::query("UPDATE projects SET default_pool_size = $1, updated_at = now() WHERE id = $2")
            .bind(pool_size)
            .bind(id)
            .execute(&state.db)
            .await?;
    }

    let updated = sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE id = $1")
        .bind(id)
        .fetch_one(&state.db)
        .await?;

    // Recreate PgBouncer container if project is running
    if updated.status == "running"
        && let Some(old_pgb_id) = &updated.pgbouncer_container_id
    {
        let state_clone = state.clone();
        let old_pgb_id = old_pgb_id.clone();
        let pgbouncer_port = updated.pgbouncer_port.unwrap_or(0) as u16;
        let container_name = updated.container_name.clone();
        let db_name = updated.db_name.clone();
        let db_user = updated.db_user.clone();
        let db_password = updated.db_password.clone();
        let pool_mode = updated.pool_mode.clone();
        let max_client_conn = updated.max_client_conn;
        let default_pool_size = updated.default_pool_size;
        let network_id = updated.docker_network_id.clone();
        let project_id = updated.id;

        tokio::spawn(async move {
            tracing::info!("Recreating PgBouncer for project {}", project_id);
            let _ = docker::remove_container(&state_clone.docker, &old_pgb_id).await;

            let pgbouncer_name = format!(
                "pgbouncer-{}",
                Uuid::new_v4()
                    .to_string()
                    .replace('-', "")
                    .chars()
                    .take(12)
                    .collect::<String>()
            );

            let pgbouncer_config = docker::PgBouncerConfig {
                container_name: pgbouncer_name,
                port: pgbouncer_port,
                backend_host: container_name,
                backend_port: 5432,
                backend_db: db_name,
                backend_user: db_user,
                backend_password: db_password,
                pool_mode,
                max_client_conn,
                default_pool_size,
                network_id,
            };

            match docker::create_pgbouncer_container(&state_clone.docker, &pgbouncer_config).await {
                Ok(new_id) => {
                    let _ = sqlx::query(
                        "UPDATE projects SET pgbouncer_container_id = $1, updated_at = now() WHERE id = $2",
                    )
                    .bind(&new_id)
                    .bind(project_id)
                    .execute(&state_clone.db)
                    .await;
                    tracing::info!("PgBouncer recreated for project {}", project_id);
                }
                Err(e) => {
                    tracing::error!("Failed to recreate PgBouncer: {}", e);
                }
            }
        });
    }

    Ok(Json(PoolSettingsResponse {
        pool_mode: updated.pool_mode,
        max_client_conn: updated.max_client_conn,
        default_pool_size: updated.default_pool_size,
        pgbouncer_port: updated.pgbouncer_port,
    }))
}

// ---------------------------------------------------------------------------
// Environment Endpoints (3.4)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct EnvironmentResponse {
    id: String,
    project_id: String,
    name: String,
    endpoint_type: String,
    connection_string: String,
    is_default: bool,
}

impl EnvironmentResponse {
    fn from(env: &ProjectEnvironment) -> Self {
        Self {
            id: env.id.to_string(),
            project_id: env.project_id.to_string(),
            name: env.name.clone(),
            endpoint_type: env.endpoint_type.clone(),
            connection_string: env.connection_string.clone(),
            is_default: env.is_default,
        }
    }
}

async fn list_environments(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<EnvironmentResponse>>, AppError> {
    fetch_project_for(&state.db, project_id, user_id, Access::Read).await?;

    let environments = sqlx::query_as::<_, ProjectEnvironment>(
        "SELECT * FROM project_environments WHERE project_id = $1 ORDER BY is_default DESC, name",
    )
    .bind(project_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(
        environments.iter().map(EnvironmentResponse::from).collect(),
    ))
}

#[derive(Deserialize, Validate)]
struct CreateEnvironmentRequest {
    #[validate(length(min = 1, max = 50))]
    name: String,
    #[validate(length(min = 1, max = 20))]
    endpoint_type: Option<String>,
    is_default: Option<bool>,
}

async fn create_environment(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
    Path(project_id): Path<Uuid>,
    Json(req): Json<CreateEnvironmentRequest>,
) -> Result<Json<EnvironmentResponse>, AppError> {
    req.validate()
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    let project = fetch_project_for(&state.db, project_id, user_id, Access::Manage).await?;

    let endpoint_type = req.endpoint_type.unwrap_or_else(|| "direct".to_string());
    if !matches!(endpoint_type.as_str(), "direct" | "pooled") {
        return Err(AppError::BadRequest(
            "endpoint_type must be 'direct' or 'pooled'".into(),
        ));
    }

    let connection_string = if endpoint_type == "pooled" {
        project.pgbouncer_port.map_or(
            Err(AppError::BadRequest(
                "Pooled endpoint not available for this project".into(),
            )),
            |p| {
                Ok(format!(
                    "postgres://{}:{}@{}:{}/{}",
                    project.db_user, project.db_password, state.config.public_host, p, project.db_name
                ))
            },
        )?
    } else {
        format!(
            "postgres://{}:{}@{}:{}/{}",
            project.db_user, project.db_password, state.config.public_host, project.port, project.db_name
        )
    };

    let is_default = req.is_default.unwrap_or(false);

    if is_default {
        sqlx::query("UPDATE project_environments SET is_default = false WHERE project_id = $1")
            .bind(project_id)
            .execute(&state.db)
            .await?;
    }

    let env = sqlx::query_as::<_, ProjectEnvironment>(
        "INSERT INTO project_environments (project_id, name, endpoint_type, connection_string, is_default)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING *",
    )
    .bind(project_id)
    .bind(&req.name)
    .bind(&endpoint_type)
    .bind(&connection_string)
    .bind(is_default)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(EnvironmentResponse::from(&env)))
}

async fn get_environment(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
    Path((project_id, env_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<EnvironmentResponse>, AppError> {
    fetch_project_for(&state.db, project_id, user_id, Access::Read).await?;

    let env = sqlx::query_as::<_, ProjectEnvironment>(
        "SELECT * FROM project_environments WHERE id = $1 AND project_id = $2",
    )
    .bind(env_id)
    .bind(project_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Json(EnvironmentResponse::from(&env)))
}

async fn delete_environment(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
    Path((project_id, env_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, AppError> {
    fetch_project_for(&state.db, project_id, user_id, Access::Manage).await?;

    let result = sqlx::query("DELETE FROM project_environments WHERE id = $1 AND project_id = $2")
        .bind(env_id)
        .bind(project_id)
        .execute(&state.db)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(Json(serde_json::json!({ "deleted": true })))
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

async fn find_available_port(state: &AppState, exclude: &[i32]) -> Result<i32, AppError> {
    let used_ports: Vec<i32> =
        sqlx::query_scalar("SELECT port FROM projects WHERE status != 'deleted'")
            .fetch_all(&state.db)
            .await?;

    let pgb_ports: Vec<i32> =
        sqlx::query_scalar("SELECT pgbouncer_port FROM projects WHERE status != 'deleted' AND pgbouncer_port IS NOT NULL")
            .fetch_all(&state.db)
            .await?;

    let mut port = 15432;
    loop {
        if port > 25432 {
            return Err(AppError::Internal("No available ports".into()));
        }
        if used_ports.contains(&port) || pgb_ports.contains(&port) || exclude.contains(&port) {
            port += 1;
            continue;
        }
        match tokio::net::TcpListener::bind(("0.0.0.0", port as u16)).await {
            Ok(_) => break,
            Err(_) => {
                port += 1;
                continue;
            }
        }
    }
    Ok(port)
}
