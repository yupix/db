//! `pg_dump`/`pg_restore` against a project's container, writing archives to
//! the host filesystem (outside any container) so they survive container and
//! project deletion until explicitly pruned.

use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

use crate::error::AppError;

const DUMP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60 * 30);

/// Run `pg_dump -Fc` (custom format, compressed, restorable with pg_restore)
/// against the container and write the archive to `dest_path` on the host.
pub async fn dump_to_file(
    container_id: &str,
    db_user: &str,
    db_name: &str,
    db_password: &str,
    dest_path: &Path,
) -> Result<u64, AppError> {
    if let Some(parent) = dest_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to create backup dir: {}", e)))?;
    }

    let mut cmd = tokio::process::Command::new("docker");
    cmd.args([
        "exec",
        container_id,
        "pg_dump",
        "-U",
        db_user,
        "-d",
        db_name,
        "-Fc", // custom format: compressed, restorable with pg_restore
    ])
    .env("PGPASSWORD", db_password)
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| AppError::Internal(format!("Failed to start pg_dump: {}", e)))?;

    let mut stdout = child.stdout.take().expect("stdout piped");
    let mut file = tokio::fs::File::create(dest_path)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to create backup file: {}", e)))?;

    let copy_result = tokio::time::timeout(DUMP_TIMEOUT, tokio::io::copy(&mut stdout, &mut file))
        .await
        .map_err(|_| AppError::Internal("pg_dump timed out".into()))?;

    let bytes_written =
        copy_result.map_err(|e| AppError::Internal(format!("pg_dump I/O error: {}", e)))?;

    let output = tokio::time::timeout(DUMP_TIMEOUT, child.wait_with_output())
        .await
        .map_err(|_| AppError::Internal("pg_dump timed out".into()))?
        .map_err(|e| AppError::Internal(format!("Failed to wait for pg_dump: {}", e)))?;

    if !output.status.success() {
        let _ = tokio::fs::remove_file(dest_path).await;
        return Err(AppError::Internal(format!(
            "pg_dump failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    Ok(bytes_written)
}

/// Restore a `pg_dump -Fc` archive into the target container/database via
/// `pg_restore`. The target database must already exist and is NOT dropped
/// first; `--clean --if-exists` drops conflicting objects before recreating
/// them, matching how Neon-style branch resets behave (data replaced in place).
pub async fn restore_from_file(
    container_id: &str,
    db_user: &str,
    db_name: &str,
    db_password: &str,
    src_path: &PathBuf,
) -> Result<(), AppError> {
    let mut cmd = tokio::process::Command::new("docker");
    cmd.args([
        "exec",
        "-i",
        container_id,
        "pg_restore",
        "-U",
        db_user,
        "-d",
        db_name,
        "--clean",
        "--if-exists",
        "--no-owner",
    ])
    .env("PGPASSWORD", db_password)
    .stdin(std::process::Stdio::piped())
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| AppError::Internal(format!("Failed to start pg_restore: {}", e)))?;

    let mut file = tokio::fs::File::open(src_path)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to open backup file: {}", e)))?;

    {
        let mut stdin = child.stdin.take().expect("stdin piped");
        tokio::time::timeout(DUMP_TIMEOUT, tokio::io::copy(&mut file, &mut stdin))
            .await
            .map_err(|_| AppError::Internal("pg_restore timed out".into()))?
            .map_err(|e| AppError::Internal(format!("pg_restore I/O error: {}", e)))?;
        stdin
            .shutdown()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to close pg_restore stdin: {}", e)))?;
    }

    let output = tokio::time::timeout(DUMP_TIMEOUT, child.wait_with_output())
        .await
        .map_err(|_| AppError::Internal("pg_restore timed out".into()))?
        .map_err(|e| AppError::Internal(format!("Failed to wait for pg_restore: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // pg_restore exits non-zero even for purely cosmetic issues (e.g. a
        // missing role from --no-owner skips), which it reports as
        // "pg_restore: warning: ...". Only treat the run as failed if stderr
        // contains something other than those warning lines — a genuine error
        // (bad archive, connection failure, etc.) always includes non-warning
        // text.
        let only_warnings = stderr
            .lines()
            .all(|l| l.trim().is_empty() || l.trim_start().starts_with("pg_restore: warning:"));

        if !only_warnings {
            return Err(AppError::Internal(format!(
                "pg_restore failed: {}",
                stderr.trim()
            )));
        }
        tracing::warn!("pg_restore completed with warnings: {}", stderr.trim());
    }

    Ok(())
}
