//! Background metrics collection.
//!
//! Two periodic tasks run for the lifetime of the process:
//! - the **collector** polls `docker stats` for every running project and
//!   stores a raw sample, and
//! - the **rollup** aggregates completed-hour raw samples into hourly buckets
//!   and prunes data past its retention window.

use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

use crate::orchestrator::docker;
use crate::state::AppState;

const COLLECT_INTERVAL: Duration = Duration::from_secs(30);
const ROLLUP_INTERVAL: Duration = Duration::from_secs(600); // 10 minutes
const RAW_RETENTION: &str = "24 hours";
const ROLLUP_RETENTION: &str = "30 days";
/// Upper bound on a single container's stats call so one hung container can't
/// stall the whole collection cycle.
const STATS_TIMEOUT: Duration = Duration::from_secs(10);

/// Spawn the collector and rollup loops. Returns immediately.
pub fn spawn(state: Arc<AppState>) {
    let collector_state = state.clone();
    tokio::spawn(async move { collector_loop(collector_state).await });
    tokio::spawn(async move { rollup_loop(state).await });
}

async fn collector_loop(state: Arc<AppState>) {
    let mut ticker = tokio::time::interval(COLLECT_INTERVAL);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        ticker.tick().await;
        if let Err(e) = collect_once(&state).await {
            tracing::warn!("metrics collection cycle failed: {}", e);
        }
    }
}

async fn collect_once(state: &AppState) -> Result<(), sqlx::Error> {
    let projects = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, container_id FROM projects
         WHERE status = 'running' AND container_id IS NOT NULL",
    )
    .fetch_all(&state.db)
    .await?;

    // Collect all projects concurrently: `docker stats` blocks ~1s per call
    // (it needs a two-read cycle for the CPU delta), so serial collection would
    // scale as O(N seconds) and could exceed COLLECT_INTERVAL. A per-call
    // timeout also keeps one hung container from starving the others.
    let tasks = projects
        .into_iter()
        .map(|(project_id, container_id)| collect_project(state, project_id, container_id));
    futures_util::future::join_all(tasks).await;

    Ok(())
}

async fn collect_project(state: &AppState, project_id: Uuid, container_id: String) {
    let stats = tokio::time::timeout(
        STATS_TIMEOUT,
        docker::container_stats(&state.docker, &container_id),
    )
    .await;

    let s = match stats {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => {
            // A container may have just stopped; expected race, debug only.
            tracing::debug!("stats unavailable for project {}: {}", project_id, e);
            return;
        }
        Err(_) => {
            tracing::warn!("stats timed out for project {}", project_id);
            return;
        }
    };

    let res = sqlx::query(
        "INSERT INTO project_metrics
           (project_id, cpu_pct, mem_used_bytes, mem_limit_bytes,
            net_rx_bytes, net_tx_bytes, block_read_bytes, block_write_bytes)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(project_id)
    .bind(s.cpu_pct)
    .bind(s.mem_used_bytes)
    .bind(s.mem_limit_bytes)
    .bind(s.net_rx_bytes)
    .bind(s.net_tx_bytes)
    .bind(s.block_read_bytes)
    .bind(s.block_write_bytes)
    .execute(&state.db)
    .await;
    if let Err(e) = res {
        tracing::warn!("failed to store metrics for {}: {}", project_id, e);
    }
}

async fn rollup_loop(state: Arc<AppState>) {
    let mut ticker = tokio::time::interval(ROLLUP_INTERVAL);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        ticker.tick().await;
        if let Err(e) = rollup_once(&state).await {
            tracing::warn!("metrics rollup cycle failed: {}", e);
        }
    }
}

async fn rollup_once(state: &AppState) -> Result<(), sqlx::Error> {
    // Aggregate completed hours into the rollup table. We only look at the last
    // few hours of raw data: older buckets are already final, so re-aggregating
    // all 24h every cycle would be wasted work that grows with project count.
    // The lower bound is wider than the rollup interval so a missed cycle still
    // gets picked up. Idempotent via ON CONFLICT.
    sqlx::query(
        "INSERT INTO project_metrics_rollup
           (project_id, bucket, samples, cpu_pct_avg, cpu_pct_max,
            mem_used_avg, mem_used_max, mem_limit_bytes,
            net_rx_bytes, net_tx_bytes, block_read_bytes, block_write_bytes)
         SELECT project_id,
                date_trunc('hour', ts) AS bucket,
                count(*),
                avg(cpu_pct),
                max(cpu_pct),
                avg(mem_used_bytes)::bigint,
                max(mem_used_bytes),
                max(mem_limit_bytes),
                max(net_rx_bytes),
                max(net_tx_bytes),
                max(block_read_bytes),
                max(block_write_bytes)
         FROM project_metrics
         WHERE ts >= date_trunc('hour', now()) - interval '3 hours'
           AND ts < date_trunc('hour', now())
         GROUP BY project_id, bucket
         ON CONFLICT (project_id, bucket) DO UPDATE SET
            samples = EXCLUDED.samples,
            cpu_pct_avg = EXCLUDED.cpu_pct_avg,
            cpu_pct_max = EXCLUDED.cpu_pct_max,
            mem_used_avg = EXCLUDED.mem_used_avg,
            mem_used_max = EXCLUDED.mem_used_max,
            mem_limit_bytes = EXCLUDED.mem_limit_bytes,
            net_rx_bytes = EXCLUDED.net_rx_bytes,
            net_tx_bytes = EXCLUDED.net_tx_bytes,
            block_read_bytes = EXCLUDED.block_read_bytes,
            block_write_bytes = EXCLUDED.block_write_bytes",
    )
    .execute(&state.db)
    .await?;

    sqlx::query(&format!(
        "DELETE FROM project_metrics WHERE ts < now() - interval '{RAW_RETENTION}'"
    ))
    .execute(&state.db)
    .await?;

    sqlx::query(&format!(
        "DELETE FROM project_metrics_rollup WHERE bucket < now() - interval '{ROLLUP_RETENTION}'"
    ))
    .execute(&state.db)
    .await?;

    Ok(())
}
