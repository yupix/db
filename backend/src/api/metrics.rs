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

    let points = if use_rollup {
        let rows = sqlx::query_as::<_, RollupRow>(&format!(
            "SELECT bucket, cpu_pct_avg, mem_used_avg, mem_limit_bytes,
                    net_rx_bytes, net_tx_bytes, block_read_bytes, block_write_bytes
             FROM project_metrics_rollup
             WHERE project_id = $1 AND bucket >= now() - interval '{interval}'
             ORDER BY bucket"
        ))
        .bind(project_id)
        .fetch_all(&state.db)
        .await?;

        rows.into_iter()
            .map(|r| MetricPoint {
                ts: r.bucket.to_rfc3339(),
                cpu_pct: r.cpu_pct_avg,
                mem_used_bytes: r.mem_used_avg,
                mem_limit_bytes: r.mem_limit_bytes,
                net_rx_bytes: r.net_rx_bytes,
                net_tx_bytes: r.net_tx_bytes,
                block_read_bytes: r.block_read_bytes,
                block_write_bytes: r.block_write_bytes,
            })
            .collect()
    } else {
        let rows = sqlx::query_as::<_, RawRow>(&format!(
            "SELECT ts, cpu_pct, mem_used_bytes, mem_limit_bytes,
                    net_rx_bytes, net_tx_bytes, block_read_bytes, block_write_bytes
             FROM project_metrics
             WHERE project_id = $1 AND ts >= now() - interval '{interval}'
             ORDER BY ts"
        ))
        .bind(project_id)
        .fetch_all(&state.db)
        .await?;

        rows.into_iter()
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
            .collect()
    };

    Ok(Json(MetricsResponse {
        range,
        resolution: if use_rollup { "rollup" } else { "raw" }.to_string(),
        points,
    }))
}

#[derive(sqlx::FromRow)]
struct RawRow {
    ts: DateTime<Utc>,
    cpu_pct: f64,
    mem_used_bytes: i64,
    mem_limit_bytes: i64,
    net_rx_bytes: i64,
    net_tx_bytes: i64,
    block_read_bytes: i64,
    block_write_bytes: i64,
}

#[derive(sqlx::FromRow)]
struct RollupRow {
    bucket: DateTime<Utc>,
    cpu_pct_avg: f64,
    mem_used_avg: i64,
    mem_limit_bytes: i64,
    net_rx_bytes: i64,
    net_tx_bytes: i64,
    block_read_bytes: i64,
    block_write_bytes: i64,
}
