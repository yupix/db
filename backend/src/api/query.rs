use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::{Path, State},
    http::HeaderMap,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::db::access::{Access, fetch_project_for};
use crate::error::AppError;
use crate::state::AppState;

const ACCESS_TOKEN_COOKIE: &str = "access_token";

pub async fn query_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
    Path(project_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let token = extract_cookie(&headers, ACCESS_TOKEN_COOKIE).ok_or(AppError::Unauthorized)?;

    let claims = crate::auth::jwt::verify(&token, &state.config.jwt_secret)?;
    if claims.token_type != crate::auth::jwt::TokenType::Access {
        return Err(AppError::Unauthorized);
    }

    let user_id: Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;

    let project = fetch_project_for(&state.db, project_id, user_id, Access::Read).await?;

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

fn extract_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|c| {
                let c = c.trim();
                c.strip_prefix(&format!("{}=", name)).map(|s| s.to_string())
            })
        })
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
            Ok(Message::Close(_)) => break,
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

#[derive(Debug, Clone, Copy, PartialEq)]
enum QueryKind {
    Select,
    Explain,
    Mutation,
}

fn classify_query(query: &str) -> QueryKind {
    let upper = query.trim().to_uppercase();
    if upper.starts_with("EXPLAIN") || upper.starts_with("SHOW") || upper.starts_with("TABLE") {
        QueryKind::Explain
    } else if upper.starts_with("SELECT")
        || upper.starts_with("WITH")
        || upper.starts_with("VALUES")
        || upper.starts_with("TABLE")
    {
        QueryKind::Select
    } else {
        QueryKind::Mutation
    }
}

async fn execute_query_via_docker(
    container_id: &str,
    db_name: &str,
    db_user: &str,
    db_password: &str,
    query: &str,
) -> Result<QueryResult, String> {
    let kind = classify_query(query);

    match kind {
        QueryKind::Select => {
            // SELECT/CTE: use COPY ... TO STDOUT WITH CSV HEADER for structured output
            let wrapped = format!(
                "COPY ({}) TO STDOUT WITH CSV HEADER",
                query.trim().trim_end_matches(';')
            );
            let output =
                run_psql_stdin(container_id, db_user, db_name, db_password, &wrapped).await?;

            if !output.status.success() {
                return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            parse_csv_output(&stdout)
        }
        QueryKind::Explain => {
            // EXPLAIN/SHOW/TABLE: run via stdin, return as text rows
            let output = run_psql_stdin(container_id, db_user, db_name, db_password, query).await?;

            if !output.status.success() {
                return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            let lines: Vec<&str> = stdout.lines().collect();

            if lines.is_empty() {
                return Ok(QueryResult {
                    columns: vec![],
                    rows: vec![],
                    rows_affected: None,
                });
            }

            // Return as single-column "result" table
            Ok(QueryResult {
                columns: vec!["Query Plan".to_string()],
                rows: lines
                    .into_iter()
                    .map(|line| vec![serde_json::Value::String(line.to_string())])
                    .collect(),
                rows_affected: None,
            })
        }
        QueryKind::Mutation => {
            // INSERT/UPDATE/DELETE/DDL: run via stdin, parse rows affected
            let output = run_psql_stdin(container_id, db_user, db_name, db_password, query).await?;

            if !output.status.success() {
                return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            let trimmed = stdout.trim();

            let rows_affected = parse_rows_affected(trimmed);

            Ok(QueryResult {
                columns: vec![],
                rows: vec![],
                rows_affected,
            })
        }
    }
}

async fn run_psql_stdin(
    container_id: &str,
    db_user: &str,
    db_name: &str,
    db_password: &str,
    sql: &str,
) -> Result<std::process::Output, String> {
    let mut child = tokio::process::Command::new("docker")
        .args([
            "exec",
            "-i",
            container_id,
            "psql",
            "-U",
            db_user,
            "-d",
            db_name,
            "-q",
            "-v",
            "ON_ERROR_STOP=1",
        ])
        .env("PGPASSWORD", db_password)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start psql: {}", e))?;

    {
        use tokio::io::AsyncWriteExt;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(sql.as_bytes())
                .await
                .map_err(|e| format!("Failed to write query: {}", e))?;
            stdin
                .write_all(b"\n")
                .await
                .map_err(|e| format!("Failed to write newline: {}", e))?;
        }
    }

    child
        .wait_with_output()
        .await
        .map_err(|e| format!("Failed to wait for psql: {}", e))
}

fn parse_rows_affected(output: &str) -> Option<u64> {
    // psql outputs lines like "INSERT 0 1", "UPDATE 5", "DELETE 3", "CREATE TABLE"
    // For INSERT: format is "INSERT <oid> <count>", we want the last number
    // For UPDATE/DELETE: format is "UPDATE <count>" or "DELETE <count>"
    // For DDL: no number, return None
    let line = output.lines().next()?;
    let parts: Vec<&str> = line.split_whitespace().collect();

    // Try to find the last numeric field
    for part in parts.iter().rev() {
        if let Ok(n) = part.parse::<u64>() {
            return Some(n);
        }
    }

    None
}

fn parse_csv_output(csv: &str) -> Result<QueryResult, String> {
    if csv.trim().is_empty() {
        return Ok(QueryResult {
            columns: vec![],
            rows: vec![],
            rows_affected: None,
        });
    }

    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(csv.as_bytes());

    let columns: Vec<String> = reader
        .headers()
        .map_err(|e| format!("Failed to parse CSV headers: {}", e))?
        .iter()
        .map(|h| h.to_string())
        .collect();

    let mut rows = Vec::new();
    for result in reader.records() {
        let record = result.map_err(|e| format!("Failed to parse CSV row: {}", e))?;
        let row: Vec<serde_json::Value> = record
            .iter()
            .map(|field| {
                if field.is_empty() || field.eq_ignore_ascii_case("null") {
                    serde_json::Value::Null
                } else {
                    serde_json::Value::String(field.to_string())
                }
            })
            .collect();
        rows.push(row);
    }

    Ok(QueryResult {
        columns,
        rows,
        rows_affected: None,
    })
}
