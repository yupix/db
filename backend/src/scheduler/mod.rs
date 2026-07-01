//! Scheduled backup snapshots (Phase 8.1) and retention pruning (8.5).
//!
//! One hourly loop: for each project with an enabled `backup_policies` row
//! whose `schedule_hour` matches the current UTC hour, take a `pg_dump`
//! snapshot if one hasn't already been taken in the last ~23h (so a restart
//! near the boundary can't double-fire), then prune old snapshots per the
//! project's retention settings.

use chrono::{Datelike, Timelike};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

use crate::api::backups::spawn_dump_task;
use crate::state::AppState;

const CHECK_INTERVAL: Duration = Duration::from_secs(60 * 60); // hourly

pub fn spawn(state: Arc<AppState>) {
    tokio::spawn(async move { run_loop(state).await });
}

async fn run_loop(state: Arc<AppState>) {
    let mut ticker = tokio::time::interval(CHECK_INTERVAL);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        ticker.tick().await;
        if let Err(e) = run_once(&state).await {
            tracing::warn!("backup scheduler cycle failed: {}", e);
        }
    }
}

#[derive(sqlx::FromRow)]
struct DueProject {
    project_id: Uuid,
    container_id: Option<String>,
    status: String,
    db_user: String,
    db_name: String,
    db_password: String,
    daily_keep: i32,
    weekly_keep: i32,
}

async fn run_once(state: &Arc<AppState>) -> Result<(), sqlx::Error> {
    let current_hour = chrono::Utc::now().hour() as i32;

    let due = sqlx::query_as::<_, DueProject>(
        "SELECT p.id AS project_id, p.container_id, p.status,
                p.db_user, p.db_name, p.db_password,
                bp.daily_keep, bp.weekly_keep
         FROM backup_policies bp
         JOIN projects p ON p.id = bp.project_id
         WHERE bp.enabled = true
           AND bp.schedule_hour = $1
           AND p.status != 'deleted'
           AND NOT EXISTS (
             SELECT 1 FROM backups b
             WHERE b.project_id = p.id
               AND b.kind = 'scheduled'
               AND b.created_at > now() - interval '23 hours'
           )",
    )
    .bind(current_hour)
    .fetch_all(&state.db)
    .await?;

    for project in due {
        if project.status != "running" {
            tracing::debug!(
                "Skipping scheduled backup for {}: project not running",
                project.project_id
            );
            continue;
        }
        let Some(container_id) = project.container_id else {
            continue;
        };

        let file_name = format!("{}-{}.dump", project.project_id, Uuid::new_v4());
        let file_path = PathBuf::from(&state.config.backup_dir).join(&file_name);

        let record = sqlx::query_as::<_, (Uuid,)>(
            "INSERT INTO backups (project_id, file_path, status, kind)
             VALUES ($1, $2, 'creating', 'scheduled') RETURNING id",
        )
        .bind(project.project_id)
        .bind(file_path.to_string_lossy().to_string())
        .fetch_one(&state.db)
        .await?;

        spawn_dump_task(
            state.clone(),
            record.0,
            container_id,
            project.db_user,
            project.db_name,
            project.db_password,
            file_path,
        );

        if let Err(e) = prune_backups(
            state,
            project.project_id,
            project.daily_keep,
            project.weekly_keep,
        )
        .await
        {
            tracing::warn!("retention prune failed for {}: {}", project.project_id, e);
        }
    }

    Ok(())
}

#[derive(sqlx::FromRow)]
struct BackupRow {
    id: Uuid,
    file_path: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

/// Grandfather-father-son style retention: keep the most recent `daily_keep`
/// completed scheduled backups, plus the newest backup of each of the next
/// `weekly_keep` distinct ISO weeks further back. Everything else (row + file)
/// is deleted. Manual backups are never touched by this policy.
async fn prune_backups(
    state: &Arc<AppState>,
    project_id: Uuid,
    daily_keep: i32,
    weekly_keep: i32,
) -> Result<(), sqlx::Error> {
    let backups = sqlx::query_as::<_, BackupRow>(
        "SELECT id, file_path, created_at FROM backups
         WHERE project_id = $1 AND kind = 'scheduled' AND status = 'completed'
         ORDER BY created_at DESC",
    )
    .bind(project_id)
    .fetch_all(&state.db)
    .await?;

    let daily_keep = daily_keep.max(0) as usize;
    let weekly_keep = weekly_keep.max(0) as usize;

    let mut kept_ids: HashSet<Uuid> = backups.iter().take(daily_keep).map(|b| b.id).collect();

    let mut seen_weeks: HashSet<(i32, u32)> = HashSet::new();
    for b in backups.iter().skip(daily_keep) {
        if seen_weeks.len() >= weekly_keep {
            break; // remaining rows are strictly older; no more weeks to keep
        }
        let week = iso_week(b.created_at);
        if seen_weeks.insert(week) {
            kept_ids.insert(b.id);
        }
    }

    for b in &backups {
        if !kept_ids.contains(&b.id) {
            let _ = tokio::fs::remove_file(&b.file_path).await;
            sqlx::query("DELETE FROM backups WHERE id = $1")
                .bind(b.id)
                .execute(&state.db)
                .await?;
        }
    }

    Ok(())
}

fn iso_week(t: chrono::DateTime<chrono::Utc>) -> (i32, u32) {
    let iso = t.iso_week();
    (iso.year(), iso.week())
}
