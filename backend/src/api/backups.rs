use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;
use validator::Validate;

use crate::auth::jwt::AuthUser;
use crate::db::access::{Access, fetch_project_for};
use crate::db::models::{Backup, BackupPolicy};
use crate::error::AppError;
use crate::orchestrator::backup;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct BackupResponse {
    id: String,
    project_id: String,
    file_path: String,
    size_bytes: Option<i64>,
    status: String,
    kind: String,
    error: Option<String>,
    created_at: String,
    completed_at: Option<String>,
}

impl BackupResponse {
    fn from(b: &Backup) -> Self {
        Self {
            id: b.id.to_string(),
            project_id: b.project_id.to_string(),
            file_path: b.file_path.clone(),
            size_bytes: b.size_bytes,
            status: b.status.clone(),
            kind: b.kind.clone(),
            error: b.error.clone(),
            created_at: b.created_at.to_rfc3339(),
            completed_at: b.completed_at.map(|t| t.to_rfc3339()),
        }
    }
}

#[derive(Serialize)]
pub struct BackupPolicyResponse {
    enabled: bool,
    schedule_hour: i32,
    daily_keep: i32,
    weekly_keep: i32,
}

impl BackupPolicyResponse {
    fn from(p: &BackupPolicy) -> Self {
        Self {
            enabled: p.enabled,
            schedule_hour: p.schedule_hour,
            daily_keep: p.daily_keep,
            weekly_keep: p.weekly_keep,
        }
    }
}

fn default_policy() -> BackupPolicyResponse {
    BackupPolicyResponse {
        enabled: false,
        schedule_hour: 3,
        daily_keep: 7,
        weekly_keep: 4,
    }
}

// ---------------------------------------------------------------------------
// Backup handlers
// ---------------------------------------------------------------------------

pub async fn list_backups(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<BackupResponse>>, AppError> {
    fetch_project_for(&state.db, project_id, user_id, Access::Read).await?;

    let backups = sqlx::query_as::<_, Backup>(
        "SELECT * FROM backups WHERE project_id = $1 ORDER BY created_at DESC LIMIT 100",
    )
    .bind(project_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(backups.iter().map(BackupResponse::from).collect()))
}

pub async fn create_backup(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<Json<BackupResponse>, AppError> {
    let project = fetch_project_for(&state.db, project_id, user_id, Access::Manage).await?;

    if project.status != "running" {
        return Err(AppError::BadRequest(
            "Project must be running to create a backup".into(),
        ));
    }
    let container_id = project
        .container_id
        .clone()
        .ok_or_else(|| AppError::BadRequest("Project has no container".into()))?;

    let file_name = format!("{}-{}.dump", project_id, Uuid::new_v4());
    let file_path = PathBuf::from(&state.config.backup_dir).join(&file_name);

    let record = sqlx::query_as::<_, Backup>(
        "INSERT INTO backups (project_id, file_path, status, kind)
         VALUES ($1, $2, 'creating', 'manual') RETURNING *",
    )
    .bind(project_id)
    .bind(file_path.to_string_lossy().to_string())
    .fetch_one(&state.db)
    .await?;

    spawn_dump_task(
        state.clone(),
        record.id,
        container_id,
        project.db_user.clone(),
        project.db_name.clone(),
        project.db_password.clone(),
        file_path,
    );

    Ok(Json(BackupResponse::from(&record)))
}

/// Run the actual `pg_dump` off the request path and update the backup row
/// with the outcome, mirroring how project/branch provisioning is handled
/// elsewhere in this codebase.
pub(crate) fn spawn_dump_task(
    state: Arc<AppState>,
    backup_id: Uuid,
    container_id: String,
    db_user: String,
    db_name: String,
    db_password: String,
    file_path: PathBuf,
) {
    tokio::spawn(async move {
        let result =
            backup::dump_to_file(&container_id, &db_user, &db_name, &db_password, &file_path).await;

        match result {
            Ok(size) => {
                let _ = sqlx::query(
                    "UPDATE backups SET status = 'completed', size_bytes = $1, completed_at = now()
                     WHERE id = $2",
                )
                .bind(size as i64)
                .bind(backup_id)
                .execute(&state.db)
                .await;
            }
            Err(e) => {
                tracing::error!("Backup {} failed: {}", backup_id, e);
                let _ = sqlx::query(
                    "UPDATE backups SET status = 'failed', error = $1, completed_at = now()
                     WHERE id = $2",
                )
                .bind(e.to_string())
                .bind(backup_id)
                .execute(&state.db)
                .await;
            }
        }
    });
}

pub async fn delete_backup(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
    Path((project_id, backup_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, AppError> {
    fetch_project_for(&state.db, project_id, user_id, Access::Manage).await?;

    let backup =
        sqlx::query_as::<_, Backup>("SELECT * FROM backups WHERE id = $1 AND project_id = $2")
            .bind(backup_id)
            .bind(project_id)
            .fetch_optional(&state.db)
            .await?
            .ok_or(AppError::NotFound)?;

    let _ = tokio::fs::remove_file(&backup.file_path).await;

    sqlx::query("DELETE FROM backups WHERE id = $1")
        .bind(backup_id)
        .execute(&state.db)
        .await?;

    Ok(Json(serde_json::json!({ "deleted": true })))
}

pub async fn restore_backup(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
    Path((project_id, backup_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, AppError> {
    let project = fetch_project_for(&state.db, project_id, user_id, Access::Manage).await?;

    if project.status != "running" {
        return Err(AppError::BadRequest(
            "Project must be running to restore a backup".into(),
        ));
    }

    let backup = sqlx::query_as::<_, Backup>(
        "SELECT * FROM backups WHERE id = $1 AND project_id = $2 AND status = 'completed'",
    )
    .bind(backup_id)
    .bind(project_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let container_id = project
        .container_id
        .clone()
        .ok_or_else(|| AppError::BadRequest("Project has no container".into()))?;

    backup::restore_from_file(
        &container_id,
        &project.db_user,
        &project.db_name,
        &project.db_password,
        &PathBuf::from(&backup.file_path),
    )
    .await?;

    Ok(Json(serde_json::json!({ "restored": true })))
}

// ---------------------------------------------------------------------------
// Retention policy
// ---------------------------------------------------------------------------

pub async fn get_backup_policy(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<Json<BackupPolicyResponse>, AppError> {
    fetch_project_for(&state.db, project_id, user_id, Access::Read).await?;

    let policy =
        sqlx::query_as::<_, BackupPolicy>("SELECT * FROM backup_policies WHERE project_id = $1")
            .bind(project_id)
            .fetch_optional(&state.db)
            .await?;

    Ok(Json(
        policy
            .as_ref()
            .map(BackupPolicyResponse::from)
            .unwrap_or_else(default_policy),
    ))
}

#[derive(Deserialize, Validate)]
pub struct UpdateBackupPolicyRequest {
    enabled: Option<bool>,
    #[validate(range(min = 0, max = 23))]
    schedule_hour: Option<i32>,
    #[validate(range(min = 1, max = 60))]
    daily_keep: Option<i32>,
    #[validate(range(min = 0, max = 52))]
    weekly_keep: Option<i32>,
}

pub async fn update_backup_policy(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
    Path(project_id): Path<Uuid>,
    Json(req): Json<UpdateBackupPolicyRequest>,
) -> Result<Json<BackupPolicyResponse>, AppError> {
    req.validate()
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    fetch_project_for(&state.db, project_id, user_id, Access::Manage).await?;

    let existing =
        sqlx::query_as::<_, BackupPolicy>("SELECT * FROM backup_policies WHERE project_id = $1")
            .bind(project_id)
            .fetch_optional(&state.db)
            .await?;

    let (enabled, schedule_hour, daily_keep, weekly_keep) = match &existing {
        Some(p) => (
            req.enabled.unwrap_or(p.enabled),
            req.schedule_hour.unwrap_or(p.schedule_hour),
            req.daily_keep.unwrap_or(p.daily_keep),
            req.weekly_keep.unwrap_or(p.weekly_keep),
        ),
        None => (
            req.enabled.unwrap_or(false),
            req.schedule_hour.unwrap_or(3),
            req.daily_keep.unwrap_or(7),
            req.weekly_keep.unwrap_or(4),
        ),
    };

    let policy = sqlx::query_as::<_, BackupPolicy>(
        "INSERT INTO backup_policies (project_id, enabled, schedule_hour, daily_keep, weekly_keep)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (project_id) DO UPDATE SET
            enabled = EXCLUDED.enabled,
            schedule_hour = EXCLUDED.schedule_hour,
            daily_keep = EXCLUDED.daily_keep,
            weekly_keep = EXCLUDED.weekly_keep,
            updated_at = now()
         RETURNING *",
    )
    .bind(project_id)
    .bind(enabled)
    .bind(schedule_hour)
    .bind(daily_keep)
    .bind(weekly_keep)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(BackupPolicyResponse::from(&policy)))
}
