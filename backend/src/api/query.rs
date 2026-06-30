use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::{Path, State},
    http::HeaderMap,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::db::models::Project;
use crate::error::AppError;
use crate::state::AppState;

const ACCESS_TOKEN_COOKIE: &str = "access_token";

pub async fn query_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
    Path(project_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    // Extract token from Cookie header (HttpOnly cookie)
    let token = extract_cookie(&headers, ACCESS_TOKEN_COOKIE).ok_or(AppError::Unauthorized)?;

    let claims = crate::auth::jwt::verify(&token, &state.config.jwt_secret)?;
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
    let is_select = {
        let upper = query.trim().to_uppercase();
        upper.starts_with("SELECT")
            || upper.starts_with("WITH")
            || upper.starts_with("EXPLAIN")
            || upper.starts_with("SHOW")
            || upper.starts_with("TABLE")
    };

    if is_select {
        // For SELECT/EXPLAIN: use COPY ... TO STDOUT WITH CSV HEADER via stdin
        let wrapped = format!(
            "COPY ({}) TO STDOUT WITH CSV HEADER",
            query.trim().trim_end_matches(';')
        );

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
                    .write_all(wrapped.as_bytes())
                    .await
                    .map_err(|e| format!("Failed to write query: {}", e))?;
                stdin
                    .write_all(b"\n")
                    .await
                    .map_err(|e| format!("Failed to write newline: {}", e))?;
            }
        }

        let output = child
            .wait_with_output()
            .await
            .map_err(|e| format!("Failed to wait for psql: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(stderr.trim().to_string());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        parse_csv(&stdout)
    } else {
        // For non-SELECT (INSERT/UPDATE/DELETE/DDL): execute via stdin
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
                    .write_all(query.trim().as_bytes())
                    .await
                    .map_err(|e| format!("Failed to write query: {}", e))?;
                stdin
                    .write_all(b"\n")
                    .await
                    .map_err(|e| format!("Failed to write newline: {}", e))?;
            }
        }

        let output = child
            .wait_with_output()
            .await
            .map_err(|e| format!("Failed to wait for psql: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(stderr.trim().to_string());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let trimmed = stdout.trim();

        // Try to extract rows affected from output (e.g., "INSERT 0 1")
        let rows_affected = trimmed
            .split_whitespace()
            .nth(1)
            .and_then(|v| v.parse::<u64>().ok());

        Ok(QueryResult {
            columns: vec![],
            rows: vec![],
            rows_affected,
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
