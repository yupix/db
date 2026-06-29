use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use validator::Validate;

use crate::auth::jwt::Claims;
use crate::db::models::Project;
use crate::error::AppError;
use crate::orchestrator::docker;
use crate::state::AppState;

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
}

#[derive(Serialize)]
struct ProjectResponse {
    id: String,
    name: String,
    slug: String,
    status: String,
    port: i32,
    db_name: String,
    db_user: String,
    connection_string: String,
    created_at: String,
}

impl ProjectResponse {
    fn from(project: &Project, host: &str) -> Self {
        Self {
            id: project.id.to_string(),
            name: project.name.clone(),
            slug: project.slug.clone(),
            status: project.status.clone(),
            port: project.port,
            db_name: project.db_name.clone(),
            db_user: project.db_user.clone(),
            connection_string: format!(
                "postgres://{}:{}@{}:{}/{}",
                project.db_user, project.db_password, host, project.port, project.db_name
            ),
            created_at: project.created_at.to_rfc3339(),
        }
    }
}

async fn list_projects(
    State(state): State<Arc<AppState>>,
    claims: Claims,
) -> Result<Json<Vec<ProjectResponse>>, AppError> {
    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;

    let projects = sqlx::query_as::<_, Project>(
        "SELECT * FROM projects WHERE user_id = $1 AND status != 'deleted' ORDER BY created_at DESC"
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(
        projects
            .iter()
            .map(|p| ProjectResponse::from(p, "localhost"))
            .collect(),
    ))
}

#[derive(Deserialize, Validate)]
struct CreateProjectRequest {
    #[validate(length(min = 1, max = 100))]
    name: String,
}

async fn create_project(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Json(req): Json<CreateProjectRequest>,
) -> Result<Json<ProjectResponse>, AppError> {
    req.validate()
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;

    let slug = slugify(&req.name);

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
    let db_password = generate_password();
    let container_name = format!(
        "userpg-{}",
        Uuid::new_v4()
            .to_string()
            .replace('-', "")
            .chars()
            .take(12)
            .collect::<String>()
    );

    let port = find_available_port(&state).await?;

    let project = sqlx::query_as::<_, Project>(
        "INSERT INTO projects (user_id, name, slug, container_name, db_name, db_user, db_password, port, status)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'creating')
         RETURNING *"
    )
    .bind(user_id)
    .bind(&req.name)
    .bind(&slug)
    .bind(&container_name)
    .bind(&db_name)
    .bind(&db_user)
    .bind(&db_password)
    .bind(port)
    .fetch_one(&state.db)
    .await?;

    let state_clone = state.clone();
    let project_id = project.id;
    let cname = container_name.clone();
    let dbn = db_name.clone();
    let dbu = db_user.clone();
    let dbp = db_password.clone();

    tokio::spawn(async move {
        let result = docker::create_postgres_container(
            &state_clone.docker,
            &cname,
            &dbn,
            &dbu,
            &dbp,
            port as u16,
        )
        .await;

        let new_status = match result {
            Ok(container_id) => {
                let _ = sqlx::query(
                    "UPDATE projects SET container_id = $1, status = 'running', updated_at = now() WHERE id = $2"
                )
                .bind(&container_id)
                .bind(project_id)
                .execute(&state_clone.db)
                .await;
                return;
            }
            Err(e) => {
                tracing::error!("Failed to create container: {}", e);
                "error"
            }
        };

        let _ = sqlx::query("UPDATE projects SET status = $1, updated_at = now() WHERE id = $2")
            .bind(new_status)
            .bind(project_id)
            .execute(&state_clone.db)
            .await;
    });

    Ok(Json(ProjectResponse::from(&project, "localhost")))
}

async fn get_project(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(id): Path<Uuid>,
) -> Result<Json<ProjectResponse>, AppError> {
    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;

    let project =
        sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(user_id)
            .fetch_optional(&state.db)
            .await?
            .ok_or(AppError::NotFound)?;

    Ok(Json(ProjectResponse::from(&project, "localhost")))
}

async fn delete_project(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;

    let project =
        sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(user_id)
            .fetch_optional(&state.db)
            .await?
            .ok_or(AppError::NotFound)?;

    if let Some(cid) = &project.container_id {
        let _ = docker::remove_container(&state.docker, cid).await;
    }

    sqlx::query("DELETE FROM projects WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;

    Ok(Json(serde_json::json!({ "deleted": true })))
}

async fn start_project(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(id): Path<Uuid>,
) -> Result<Json<ProjectResponse>, AppError> {
    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;

    let project =
        sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(user_id)
            .fetch_optional(&state.db)
            .await?
            .ok_or(AppError::NotFound)?;

    if let Some(cid) = &project.container_id {
        docker::start_container(&state.docker, cid).await?;
        sqlx::query("UPDATE projects SET status = 'running', updated_at = now() WHERE id = $1")
            .bind(id)
            .execute(&state.db)
            .await?;
    }

    let updated = sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE id = $1")
        .bind(id)
        .fetch_one(&state.db)
        .await?;

    Ok(Json(ProjectResponse::from(&updated, "localhost")))
}

async fn stop_project(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(id): Path<Uuid>,
) -> Result<Json<ProjectResponse>, AppError> {
    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;

    let project =
        sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(user_id)
            .fetch_optional(&state.db)
            .await?
            .ok_or(AppError::NotFound)?;

    if let Some(cid) = &project.container_id {
        docker::stop_container(&state.docker, cid).await?;
        sqlx::query("UPDATE projects SET status = 'stopped', updated_at = now() WHERE id = $1")
            .bind(id)
            .execute(&state.db)
            .await?;
    }

    let updated = sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE id = $1")
        .bind(id)
        .fetch_one(&state.db)
        .await?;

    Ok(Json(ProjectResponse::from(&updated, "localhost")))
}

#[derive(Deserialize)]
struct UpdateProjectRequest {
    name: Option<String>,
}

async fn update_project(
    State(state): State<Arc<AppState>>,
    claims: Claims,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateProjectRequest>,
) -> Result<Json<ProjectResponse>, AppError> {
    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;

    let _project =
        sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(user_id)
            .fetch_optional(&state.db)
            .await?
            .ok_or(AppError::NotFound)?;

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

    Ok(Json(ProjectResponse::from(&updated, "localhost")))
}

fn slugify(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn generate_password() -> String {
    use rand::Rng;
    const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut rng = rand::rng();
    (0..24)
        .map(|_| CHARS[rng.random_range(0..CHARS.len())] as char)
        .collect()
}

async fn find_available_port(state: &AppState) -> Result<i32, AppError> {
    let used_ports: Vec<i32> =
        sqlx::query_scalar("SELECT port FROM projects WHERE status != 'deleted'")
            .fetch_all(&state.db)
            .await?;

    let mut port = 15432;
    loop {
        if port > 25432 {
            return Err(AppError::Internal("No available ports".into()));
        }
        if used_ports.contains(&port) {
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
