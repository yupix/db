use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use validator::Validate;

use crate::auth::jwt::AuthUser;
use crate::db::access::{Access, fetch_project_for};
use crate::db::models::{Branch, Project};
use crate::error::AppError;
use crate::orchestrator::docker;
use crate::state::AppState;

#[derive(Serialize)]
pub struct BranchResponse {
    id: String,
    project_id: String,
    parent_branch_id: Option<String>,
    name: String,
    status: String,
    port: i32,
    connection_string: String,
    created_at: String,
}

impl BranchResponse {
    fn from(branch: &Branch, project: &Project, host: &str) -> Self {
        Self {
            id: branch.id.to_string(),
            project_id: branch.project_id.to_string(),
            parent_branch_id: branch.parent_branch_id.map(|id| id.to_string()),
            name: branch.name.clone(),
            status: branch.status.clone(),
            port: branch.port,
            connection_string: format!(
                "postgres://{}:{}@{}:{}/{}",
                project.db_user, project.db_password, host, branch.port, project.db_name
            ),
            created_at: branch.created_at.to_rfc3339(),
        }
    }
}

pub async fn list_branches(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<BranchResponse>>, AppError> {
    let project = fetch_project_for(&state.db, project_id, user_id, Access::Read).await?;

    let branches = sqlx::query_as::<_, Branch>(
        "SELECT * FROM branches WHERE project_id = $1 AND status != 'deleted' ORDER BY created_at",
    )
    .bind(project_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(
        branches
            .iter()
            .map(|b| BranchResponse::from(b, &project, "localhost"))
            .collect(),
    ))
}

#[derive(Deserialize, Validate)]
pub struct CreateBranchRequest {
    #[validate(length(min = 1, max = 100))]
    name: String,
    parent_branch_id: Option<Uuid>,
}

pub async fn create_branch(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
    Path(project_id): Path<Uuid>,
    Json(req): Json<CreateBranchRequest>,
) -> Result<Json<BranchResponse>, AppError> {
    req.validate()
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    let project = fetch_project_for(&state.db, project_id, user_id, Access::Manage).await?;

    if project.status != "running" {
        return Err(AppError::BadRequest(
            "Project must be running to create a branch".into(),
        ));
    }

    let parent_branch = if let Some(parent_id) = req.parent_branch_id {
        let parent =
            sqlx::query_as::<_, Branch>("SELECT * FROM branches WHERE id = $1 AND project_id = $2")
                .bind(parent_id)
                .bind(project_id)
                .fetch_optional(&state.db)
                .await?
                .ok_or(AppError::NotFound)?;

        if parent.status != "running" {
            return Err(AppError::BadRequest("Parent branch must be running".into()));
        }
        Some(parent)
    } else {
        None
    };

    let container_name = format!(
        "branch-{}",
        Uuid::new_v4()
            .to_string()
            .replace('-', "")
            .chars()
            .take(12)
            .collect::<String>()
    );

    let existing_name = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM branches WHERE project_id = $1 AND name = $2 AND status != 'deleted'",
    )
    .bind(project_id)
    .bind(&req.name)
    .fetch_one(&state.db)
    .await?;

    if existing_name > 0 {
        return Err(AppError::Conflict("Branch name already exists".into()));
    }

    let port = find_available_branch_port(&state).await?;

    let source_container_id = parent_branch
        .as_ref()
        .and_then(|b| b.container_id.clone())
        .or_else(|| project.container_id.clone())
        .ok_or(AppError::BadRequest("No source container available".into()))?;

    let branch = sqlx::query_as::<_, Branch>(
        "INSERT INTO branches (project_id, parent_branch_id, name, container_name, port, status)
         VALUES ($1, $2, $3, $4, $5, 'creating')
         RETURNING *",
    )
    .bind(project_id)
    .bind(req.parent_branch_id)
    .bind(&req.name)
    .bind(&container_name)
    .bind(port)
    .fetch_one(&state.db)
    .await?;

    let state_clone = state.clone();
    let branch_id = branch.id;
    let cname = container_name.clone();
    let db_name = project.db_name.clone();
    let db_user = project.db_user.clone();
    let db_password = project.db_password.clone();
    let network_id = project.docker_network_id.clone();
    let source_cid = source_container_id.clone();
    let source_db = project.db_name.clone();
    let source_user = project.db_user.clone();

    tokio::spawn(async move {
        let branch_config = docker::BranchConfig {
            container_name: cname,
            db_name: db_name.clone(),
            db_user: db_user.clone(),
            db_password,
            port: port as u16,
            network_id,
        };

        let result = docker::create_branch_container(
            &state_clone.docker,
            &branch_config,
            &source_cid,
            &source_db,
            &source_user,
        )
        .await;

        let new_status = match result {
            Ok(container_id) => {
                let _ = sqlx::query(
                    "UPDATE branches SET container_id = $1, status = 'running', updated_at = now() WHERE id = $2",
                )
                .bind(&container_id)
                .bind(branch_id)
                .execute(&state_clone.db)
                .await;
                return;
            }
            Err(e) => {
                tracing::error!("Failed to create branch container: {}", e);
                "error"
            }
        };

        let _ = sqlx::query("UPDATE branches SET status = $1, updated_at = now() WHERE id = $2")
            .bind(new_status)
            .bind(branch_id)
            .execute(&state_clone.db)
            .await;
    });

    Ok(Json(BranchResponse::from(&branch, &project, "localhost")))
}

pub async fn get_branch(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
    Path((project_id, branch_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<BranchResponse>, AppError> {
    let project = fetch_project_for(&state.db, project_id, user_id, Access::Read).await?;

    let branch =
        sqlx::query_as::<_, Branch>("SELECT * FROM branches WHERE id = $1 AND project_id = $2")
            .bind(branch_id)
            .bind(project_id)
            .fetch_optional(&state.db)
            .await?
            .ok_or(AppError::NotFound)?;

    Ok(Json(BranchResponse::from(&branch, &project, "localhost")))
}

pub async fn delete_branch(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
    Path((project_id, branch_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, AppError> {
    fetch_project_for(&state.db, project_id, user_id, Access::Manage).await?;

    let branch =
        sqlx::query_as::<_, Branch>("SELECT * FROM branches WHERE id = $1 AND project_id = $2")
            .bind(branch_id)
            .bind(project_id)
            .fetch_optional(&state.db)
            .await?
            .ok_or(AppError::NotFound)?;

    if branch.status == "creating" {
        return Err(AppError::Conflict(
            "Branch is currently being created. Wait for it to finish before deleting.".into(),
        ));
    }

    if let Some(cid) = &branch.container_id {
        let _ = docker::remove_container(&state.docker, cid).await;
    }

    sqlx::query("DELETE FROM branches WHERE id = $1")
        .bind(branch_id)
        .execute(&state.db)
        .await?;

    Ok(Json(serde_json::json!({ "deleted": true })))
}

#[derive(Deserialize, Validate)]
pub struct RenameBranchRequest {
    #[validate(length(min = 1, max = 100))]
    name: Option<String>,
}

pub async fn rename_branch(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
    Path((project_id, branch_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<RenameBranchRequest>,
) -> Result<Json<BranchResponse>, AppError> {
    req.validate()
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    let project = fetch_project_for(&state.db, project_id, user_id, Access::Manage).await?;

    let _branch =
        sqlx::query_as::<_, Branch>("SELECT * FROM branches WHERE id = $1 AND project_id = $2")
            .bind(branch_id)
            .bind(project_id)
            .fetch_optional(&state.db)
            .await?
            .ok_or(AppError::NotFound)?;

    if let Some(name) = &req.name {
        sqlx::query("UPDATE branches SET name = $1, updated_at = now() WHERE id = $2")
            .bind(name)
            .bind(branch_id)
            .execute(&state.db)
            .await?;
    }

    let updated = sqlx::query_as::<_, Branch>("SELECT * FROM branches WHERE id = $1")
        .bind(branch_id)
        .fetch_one(&state.db)
        .await?;

    Ok(Json(BranchResponse::from(&updated, &project, "localhost")))
}

pub async fn reset_branch(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
    Path((project_id, branch_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<BranchResponse>, AppError> {
    let project = fetch_project_for(&state.db, project_id, user_id, Access::Manage).await?;

    let branch =
        sqlx::query_as::<_, Branch>("SELECT * FROM branches WHERE id = $1 AND project_id = $2")
            .bind(branch_id)
            .bind(project_id)
            .fetch_optional(&state.db)
            .await?
            .ok_or(AppError::NotFound)?;

    if branch.status != "running" {
        return Err(AppError::BadRequest(
            "Branch must be running to reset".into(),
        ));
    }

    let branch_container_id = branch
        .container_id
        .clone()
        .ok_or(AppError::BadRequest("Branch has no container".into()))?;

    // Find the source to reset from (parent branch or main project)
    let source_container_id = if let Some(parent_id) = branch.parent_branch_id {
        let parent = sqlx::query_as::<_, Branch>("SELECT * FROM branches WHERE id = $1")
            .bind(parent_id)
            .fetch_optional(&state.db)
            .await?
            .ok_or(AppError::NotFound)?;
        parent.container_id.ok_or(AppError::BadRequest(
            "Parent branch has no container".into(),
        ))?
    } else {
        project
            .container_id
            .clone()
            .ok_or(AppError::BadRequest("Project has no container".into()))?
    };

    let state_clone = state.clone();
    let branch_id = branch.id;
    let db_name = project.db_name.clone();
    let db_user = project.db_user.clone();

    // Mark as resetting before spawning
    sqlx::query("UPDATE branches SET status = 'resetting', updated_at = now() WHERE id = $1")
        .bind(branch_id)
        .execute(&state.db)
        .await?;

    tokio::spawn(async move {
        let result = docker::reset_branch_container(
            &state_clone.docker,
            &branch_container_id,
            &source_container_id,
            &db_name,
            &db_user,
            &db_name,
            &db_user,
        )
        .await;

        let new_status = if result.is_ok() {
            "running"
        } else {
            tracing::error!("Failed to reset branch: {:?}", result.err());
            "error"
        };

        let _ = sqlx::query("UPDATE branches SET status = $1, updated_at = now() WHERE id = $2")
            .bind(new_status)
            .bind(branch_id)
            .execute(&state_clone.db)
            .await;
    });

    let updated_branch = sqlx::query_as::<_, Branch>("SELECT * FROM branches WHERE id = $1")
        .bind(branch_id)
        .fetch_one(&state.db)
        .await?;

    Ok(Json(BranchResponse::from(
        &updated_branch,
        &project,
        "localhost",
    )))
}

async fn find_available_branch_port(state: &AppState) -> Result<i32, AppError> {
    let used_ports: Vec<i32> =
        sqlx::query_scalar("SELECT port FROM branches WHERE status != 'deleted'")
            .fetch_all(&state.db)
            .await?;

    let project_ports: Vec<i32> =
        sqlx::query_scalar("SELECT port FROM projects WHERE status != 'deleted'")
            .fetch_all(&state.db)
            .await?;

    let pgb_ports: Vec<i32> =
        sqlx::query_scalar("SELECT pgbouncer_port FROM projects WHERE status != 'deleted' AND pgbouncer_port IS NOT NULL")
            .fetch_all(&state.db)
            .await?;

    let mut port = 25433;
    loop {
        if port > 35432 {
            return Err(AppError::Internal("No available ports for branches".into()));
        }
        if used_ports.contains(&port) || project_ports.contains(&port) || pgb_ports.contains(&port)
        {
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
