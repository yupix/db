use axum::{
    Json,
    extract::{Path, Query, State},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::jwt::AuthUser;
use crate::db::access::{Access, fetch_project_for};
use crate::error::AppError;
use crate::orchestrator::docker;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct MetricsQuery {
    /// One of: 1h, 6h, 24h, 7d, 30d. Defaults to 1h.
    range: Option<String>,
}

#[derive(Serialize)]
pub struct MetricPoint {
    ts: String,
    cpu_pct: f64,
    mem_used_bytes: i64,
    mem_limit_bytes: i64,
    net_rx_bytes: i64,
    net_tx_bytes: i64,
    block_read_bytes: i64,
    block_write_bytes: i64,
}

#[derive(Serialize)]
pub struct MetricsResponse {
    range: String,
    /// "raw" (per-sample) or "rollup" (hourly averages).
    resolution: String,
    points: Vec<MetricPoint>,
}

/// Resolve a range string to (interval SQL literal, whether to use the rollup).
/// Short ranges read raw samples; long ranges read the hourly rollup.
fn resolve_range(range: &str) -> Option<(&'static str, bool)> {
    match range {
        "1h" => Some(("1 hour", false)),
        "6h" => Some(("6 hours", false)),
        "24h" => Some(("24 hours", false)),
        "7d" => Some(("7 days", true)),
        "30d" => Some(("30 days", true)),
        _ => None,
    }
}

pub async fn get_metrics(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
    Path(project_id): Path<Uuid>,
    Query(q): Query<MetricsQuery>,
) -> Result<Json<MetricsResponse>, AppError> {
    fetch_project_for(&state.db, project_id, user_id, Access::Read).await?;

    let range = q.range.unwrap_or_else(|| "1h".to_string());
    let (interval, use_rollup) =
        resolve_range(&range).ok_or_else(|| AppError::BadRequest("invalid range".into()))?;

    // The rollup table's columns are aliased to match `MetricRow` so a single
    // struct and mapping serve both the raw and rollup paths. Interval literals
    // come from the fixed `resolve_range` whitelist — never user input.
    let sql = if use_rollup {
        format!(
            "SELECT bucket AS ts, cpu_pct_avg AS cpu_pct, mem_used_avg AS mem_used_bytes,
                    mem_limit_bytes, net_rx_bytes, net_tx_bytes,
                    block_read_bytes, block_write_bytes
             FROM project_metrics_rollup
             WHERE project_id = $1 AND bucket >= now() - interval '{interval}'
             ORDER BY bucket"
        )
    } else {
        format!(
            "SELECT ts, cpu_pct, mem_used_bytes, mem_limit_bytes,
                    net_rx_bytes, net_tx_bytes, block_read_bytes, block_write_bytes
             FROM project_metrics
             WHERE project_id = $1 AND ts >= now() - interval '{interval}'
             ORDER BY ts"
        )
    };

    let points = sqlx::query_as::<_, MetricRow>(&sql)
        .bind(project_id)
        .fetch_all(&state.db)
        .await?
        .into_iter()
        .map(|r| MetricPoint {
            ts: r.ts.to_rfc3339(),
            cpu_pct: r.cpu_pct,
            mem_used_bytes: r.mem_used_bytes,
            mem_limit_bytes: r.mem_limit_bytes,
            net_rx_bytes: r.net_rx_bytes,
            net_tx_bytes: r.net_tx_bytes,
            block_read_bytes: r.block_read_bytes,
            block_write_bytes: r.block_write_bytes,
        })
        .collect();

    Ok(Json(MetricsResponse {
        range,
        resolution: if use_rollup { "rollup" } else { "raw" }.to_string(),
        points,
    }))
}

#[derive(sqlx::FromRow)]
struct MetricRow {
    ts: DateTime<Utc>,
    cpu_pct: f64,
    mem_used_bytes: i64,
    mem_limit_bytes: i64,
    net_rx_bytes: i64,
    net_tx_bytes: i64,
    block_read_bytes: i64,
    block_write_bytes: i64,
}

// ---------------------------------------------------------------------------
// Query statistics via pg_stat_statements (7.1)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct QueryStat {
    query: String,
    calls: i64,
    total_exec_time_ms: f64,
    mean_exec_time_ms: f64,
    rows: i64,
}

#[derive(Serialize)]
pub struct QueryStatsResponse {
    /// False when the project isn't running or pg_stat_statements isn't enabled
    /// (e.g. a container created before Phase 7.1). The UI shows a hint instead.
    available: bool,
    stats: Vec<QueryStat>,
}

pub async fn get_query_stats(
    State(state): State<Arc<AppState>>,
    AuthUser(user_id): AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<Json<QueryStatsResponse>, AppError> {
    let project = fetch_project_for(&state.db, project_id, user_id, Access::Read).await?;

    let container_id = match &project.container_id {
        Some(c) if project.status == "running" => c,
        _ => {
            return Ok(Json(QueryStatsResponse {
                available: false,
                stats: vec![],
            }));
        }
    };

    let sql = "SELECT query, calls, total_exec_time, mean_exec_time, rows \
               FROM pg_stat_statements ORDER BY total_exec_time DESC LIMIT 20";
    let (user, db, pass) = (
        project.db_user.as_str(),
        project.db_name.as_str(),
        project.db_password.as_str(),
    );

    // Only a *missing extension* means "unavailable". Any other error (docker
    // down, timeout, auth) is a real failure and must not be disguised as
    // "recreate the project".
    let csv = match docker::psql_query_csv(container_id, user, db, pass, sql).await {
        Ok(csv) => csv,
        Err(e) if is_missing_extension(&e) => {
            // The extension may just not be created yet (e.g. a transient failure
            // during provisioning). db_user is the superuser, so try once to
            // enable it and re-run; if it still can't be created (an old container
            // without the preload), it's genuinely unavailable.
            if docker::enable_pg_stat_statements(container_id, user, db, pass)
                .await
                .is_err()
            {
                return Ok(Json(QueryStatsResponse {
                    available: false,
                    stats: vec![],
                }));
            }
            match docker::psql_query_csv(container_id, user, db, pass, sql).await {
                Ok(csv) => csv,
                Err(e) if is_missing_extension(&e) => {
                    return Ok(Json(QueryStatsResponse {
                        available: false,
                        stats: vec![],
                    }));
                }
                Err(e) => return Err(e),
            }
        }
        Err(e) => return Err(e),
    };

    Ok(Json(QueryStatsResponse {
        available: true,
        stats: parse_query_stats(&csv)?,
    }))
}

/// True when a psql error indicates the pg_stat_statements view doesn't exist,
/// as opposed to a real infrastructure failure.
fn is_missing_extension(e: &AppError) -> bool {
    matches!(e, AppError::BadRequest(msg)
        if msg.contains("pg_stat_statements") && msg.contains("does not exist"))
}

fn parse_query_stats(csv: &str) -> Result<Vec<QueryStat>, AppError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(csv.as_bytes());

    let mut stats = Vec::new();
    for record in reader.records() {
        let r = record.map_err(|e| AppError::Internal(format!("CSV parse error: {}", e)))?;
        stats.push(QueryStat {
            query: r.get(0).unwrap_or("").to_string(),
            calls: r.get(1).and_then(|v| v.parse().ok()).unwrap_or(0),
            total_exec_time_ms: r.get(2).and_then(|v| v.parse().ok()).unwrap_or(0.0),
            mean_exec_time_ms: r.get(3).and_then(|v| v.parse().ok()).unwrap_or(0.0),
            rows: r.get(4).and_then(|v| v.parse().ok()).unwrap_or(0),
        });
    }
    Ok(stats)
}
