use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::{Path, Query, State},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::db::models::Project;
use crate::error::AppError;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct WsAuthQuery {
    token: String,
}

pub async fn query_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
    Path(project_id): Path<Uuid>,
    Query(query): Query<WsAuthQuery>,
) -> Result<impl IntoResponse, AppError> {
    let claims = crate::auth::jwt::verify(&query.token, &state.config.jwt_secret)?;
    if claims.token_type != crate::auth::jwt::TokenType::Access {
        return Err(AppError::Unauthorized);
    }

    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;

    let project =
        sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE id = $1 AND user_id = $2")
            .bind(project_id)
            .bind(user_id)
            .fetch_optional(&state.db)
            .await?
            .ok_or(AppError::NotFound)?;

    if project.status != "running" {
        return Err(AppError::BadRequest("Project is not running".into()));
    }

    let container_id = project
        .container_id
        .ok_or(AppError::BadRequest("Project has no container".into()))?;

    let db_name = project.db_name.clone();
    let db_user = project.db_user.clone();
    let db_password = project.db_password.clone();

    Ok(ws.on_upgrade(move |socket| handle_ws(socket, container_id, db_name, db_user, db_password)))
}

async fn handle_ws(
    mut socket: WebSocket,
    container_id: String,
    db_name: String,
    db_user: String,
    db_password: String,
) {
    tracing::info!(
        "WebSocket query connection established for container {}",
        container_id
    );

    while let Some(msg) = socket.recv().await {
        let msg = match msg {
            Ok(Message::Text(text)) => text,
            Ok(Message::Close(_)) => {
                tracing::info!("WebSocket query connection closed");
                break;
            }
            _ => continue,
        };

        let request: WsQueryRequest = match serde_json::from_str(&msg) {
            Ok(r) => r,
            Err(e) => {
                let _ = send_json(
                    &mut socket,
                    &WsQueryResponse {
                        success: false,
                        error: Some(format!("Invalid request: {}", e)),
                        columns: None,
                        rows: None,
                        rows_affected: None,
                        execution_time_ms: None,
                    },
                )
                .await;
                continue;
            }
        };

        let start = std::time::Instant::now();

        let result = execute_query_via_docker(
            &container_id,
            &db_name,
            &db_user,
            &db_password,
            &request.query,
        )
        .await;

        let execution_time_ms = start.elapsed().as_millis() as u64;

        let response = match result {
            Ok(QueryResult {
                columns,
                rows,
                rows_affected,
            }) => WsQueryResponse {
                success: true,
                error: None,
                columns: Some(columns),
                rows: Some(rows),
                rows_affected,
                execution_time_ms: Some(execution_time_ms),
            },
            Err(e) => WsQueryResponse {
                success: false,
                error: Some(e),
                columns: None,
                rows: None,
                rows_affected: None,
                execution_time_ms: Some(execution_time_ms),
            },
        };

        if send_json(&mut socket, &response).await.is_err() {
            break;
        }
    }

    tracing::info!("WebSocket query connection ended");
}

async fn send_json(socket: &mut WebSocket, data: &WsQueryResponse) -> Result<(), axum::Error> {
    let json = serde_json::to_string(data).unwrap_or_default();
    socket.send(Message::Text(json.into())).await
}

#[derive(Deserialize)]
struct WsQueryRequest {
    query: String,
}

#[derive(Serialize)]
struct WsQueryResponse {
    success: bool,
    error: Option<String>,
    columns: Option<Vec<String>>,
    rows: Option<Vec<Vec<serde_json::Value>>>,
    rows_affected: Option<u64>,
    execution_time_ms: Option<u64>,
}

struct QueryResult {
    columns: Vec<String>,
    rows: Vec<Vec<serde_json::Value>>,
    rows_affected: Option<u64>,
}

async fn execute_query_via_docker(
    container_id: &str,
    db_name: &str,
    db_user: &str,
    db_password: &str,
    query: &str,
) -> Result<QueryResult, String> {
    // Execute query via docker exec psql with JSON output
    let psql_cmd = format!(
        "PGPASSWORD={} psql -U {} -d {} -t -A -F '\\t' -c \"{}\"",
        db_password,
        db_user,
        db_name,
        query.replace('"', "\\\"")
    );

    let output = tokio::process::Command::new("docker")
        .args(["exec", container_id, "sh", "-c", &psql_cmd])
        .output()
        .await
        .map_err(|e| format!("Failed to execute query: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(stderr.to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Check if this was a SELECT query (has output) or a non-SELECT (DDL/DML)
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Ok(QueryResult {
            columns: vec![],
            rows: vec![],
            rows_affected: None,
        });
    }

    // For non-SELECT queries (INSERT/UPDATE/DELETE), psql with -t -A outputs rows affected
    // We need a different approach: use psql with JSON output mode
    // Actually, let's use a simpler approach: parse the output as TSV
    let lines: Vec<&str> = trimmed.lines().collect();
    if lines.is_empty() {
        return Ok(QueryResult {
            columns: vec![],
            rows: vec![],
            rows_affected: None,
        });
    }

    // Try JSON output mode for better parsing
    let json_cmd = format!(
        "PGPASSWORD={} psql -U {} -d {} -t -A -c \"COPY ({}) TO STDOUT WITH CSV HEADER\"",
        db_password,
        db_user,
        db_name,
        query.replace('"', "\\\"").replace('\n', " ")
    );

    // For SELECT queries, use COPY ... TO STDOUT WITH CSV HEADER
    // For other queries, just run them normally
    let is_select = query.trim().to_uppercase().starts_with("SELECT")
        || query.trim().to_uppercase().starts_with("WITH");

    if is_select {
        let csv_output = tokio::process::Command::new("docker")
            .args(["exec", container_id, "sh", "-c", &json_cmd])
            .output()
            .await
            .map_err(|e| format!("Failed to execute query: {}", e))?;

        if !csv_output.status.success() {
            let stderr = String::from_utf8_lossy(&csv_output.stderr);
            // Fallback: try regular psql output
            return Err(stderr.to_string());
        }

        let csv_text = String::from_utf8_lossy(&csv_output.stdout);
        parse_csv(&csv_text)
    } else {
        // For non-SELECT, return the output as a message
        let lines_affected = lines.first().and_then(|l| l.parse::<u64>().ok());
        Ok(QueryResult {
            columns: vec!["result".to_string()],
            rows: vec![vec![serde_json::Value::String(stdout.trim().to_string())]],
            rows_affected: lines_affected,
        })
    }
}

fn parse_csv(csv: &str) -> Result<QueryResult, String> {
    let mut lines = csv.lines();
    let header_line = lines.next().ok_or("Empty CSV output")?;

    let columns: Vec<String> = parse_csv_line(header_line);

    let mut rows = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let values: Vec<serde_json::Value> = parse_csv_line(line)
            .into_iter()
            .map(|v| {
                if v.is_empty() || v.eq_ignore_ascii_case("null") {
                    serde_json::Value::Null
                } else {
                    serde_json::Value::String(v)
                }
            })
            .collect();
        rows.push(values);
    }

    Ok(QueryResult {
        columns,
        rows,
        rows_affected: None,
    })
}

fn parse_csv_line(line: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for c in line.chars() {
        match c {
            '"' if in_quotes => in_quotes = false,
            '"' => in_quotes = true,
            ',' if !in_quotes => {
                result.push(current.clone());
                current.clear();
            }
            _ => current.push(c),
        }
    }
    result.push(current);

    result
}
