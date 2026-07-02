#!/usr/bin/env pwsh
# ---------------------------------------------------------------------------
# 開発環境セットアップスクリプト (Windows / PowerShell)
#
#   前提チェック → .env 対話生成 → Control DB 起動 → healthy 待ち →
#   フロントエンド依存インストール まで一括で行う。
#
# 使い方:  ./scripts/setup.ps1
# ---------------------------------------------------------------------------
$ErrorActionPreference = "Stop"

# リポジトリルート (このスクリプトの1つ上) へ移動
$RepoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $RepoRoot

function Write-Step($msg) { Write-Host "`n==> $msg" -ForegroundColor Cyan }
function Write-Ok($msg)   { Write-Host "  OK  $msg" -ForegroundColor Green }
function Write-Warn($msg) { Write-Host "  !!  $msg" -ForegroundColor Yellow }

function Read-WithDefault($prompt, $default) {
    $input = Read-Host "$prompt [$default]"
    if ([string]::IsNullOrWhiteSpace($input)) { $default } else { $input }
}

function New-RandomSecret {
    -join ((65..90) + (97..122) + (48..57) | Get-Random -Count 48 | ForEach-Object { [char]$_ })
}

# --- 1. 前提コマンドの確認 -------------------------------------------------
Write-Step "前提コマンドを確認中..."
$missing = @()
foreach ($cmd in @("docker", "cargo", "pnpm", "node")) {
    if (Get-Command $cmd -ErrorAction SilentlyContinue) {
        Write-Ok "$cmd が見つかりました"
    } else {
        $missing += $cmd
        Write-Warn "$cmd が見つかりません"
    }
}
if ($missing.Count -gt 0) {
    Write-Host "`n以下のコマンドをインストールしてください: $($missing -join ', ')" -ForegroundColor Red
    Write-Host "  docker: https://docs.docker.com/get-docker/"
    Write-Host "  rust  : https://rustup.rs/"
    Write-Host "  pnpm  : https://pnpm.io/installation (Node.js 22+ 同梱)"
    exit 1
}

if (-not (docker info *>&1 | Select-String "Server")) {
    Write-Host "`nDocker デーモンが起動していません。Docker Desktop を起動してください。" -ForegroundColor Red
    exit 1
}
Write-Ok "Docker デーモンが稼働中"

# --- 2. backend/.env の生成 ------------------------------------------------
Write-Step "backend/.env を設定中..."
if (Test-Path "backend/.env") {
    Write-Ok "backend/.env は既に存在します (スキップ — 変更したい場合は削除してから再実行)"
} else {
    Write-Host ""
    Write-Host "  DB / サーバー設定を入力してください。Enter でデフォルト値を使用します。"
    Write-Host ""

    $dbHost     = Read-WithDefault "  Control DB ホスト" "localhost"
    $dbPort     = Read-WithDefault "  Control DB ポート" "5432"
    $dbName     = Read-WithDefault "  Control DB 名"     "dbcontrol"
    $dbUser     = Read-WithDefault "  DB ユーザー"       "admin"
    $dbPass     = Read-WithDefault "  DB パスワード"     "admin123"
    $apiPort    = Read-WithDefault "  バックエンドポート" "8080"
    $backupDir  = Read-WithDefault "  バックアップ保存先" "./data/backups"

    $autoSecret = New-RandomSecret
    $jwtSecret  = Read-WithDefault "  JWT シークレット (Enter で自動生成)" $autoSecret

    @"
# Database URL for Control DB
DATABASE_URL=postgres://${dbUser}:${dbPass}@${dbHost}:${dbPort}/${dbName}

# JWT Secret
JWT_SECRET=${jwtSecret}

# Server
HOST=0.0.0.0
PORT=${apiPort}

# Logging
RUST_LOG=info,backend=debug

# Host-side directory backup archives are written to
BACKUP_DIR=${backupDir}
"@ | Set-Content "backend/.env" -Encoding UTF8

    Write-Ok "backend/.env を生成しました"

    # docker-compose の DB 設定と異なる場合は警告
    if ($dbUser -ne "admin" -or $dbPass -ne "admin123" -or $dbName -ne "dbcontrol") {
        Write-Warn "docker/docker-compose.yml の DB 設定と異なる値を入力しました。"
        Write-Warn "docker-compose.yml 側も合わせて編集してください。"
    }
}

# frontend/.env.local の生成
Write-Host ""
if (Test-Path "frontend/.env.local") {
    Write-Ok "frontend/.env.local は既に存在します (スキップ)"
} else {
    $apiPort = if (Test-Path "backend/.env") {
        (Get-Content "backend/.env" | Select-String "^PORT=").ToString() -replace "^PORT=",""
    } else { "8080" }
    $apiPort = $apiPort.Trim()
    if (-not $apiPort) { $apiPort = "8080" }

    $apiUrl = Read-WithDefault "  フロント → バックエンド URL" "http://localhost:${apiPort}"
    "NEXT_PUBLIC_API_URL=${apiUrl}" | Set-Content "frontend/.env.local" -Encoding UTF8
    Write-Ok "frontend/.env.local を生成しました"
}

# --- 3. Control DB を起動 --------------------------------------------------
Write-Step "Control DB (PostgreSQL) を起動中..."
docker compose -f docker/docker-compose.yml up control-db -d
Write-Ok "control-db コンテナを起動しました"

# --- 4. healthy 待ち -------------------------------------------------------
Write-Step "Control DB の起動を待機中..."
$maxTries = 30
$ready = $false
for ($i = 1; $i -le $maxTries; $i++) {
    $health = docker inspect --format "{{.State.Health.Status}}" control-db 2>$null
    if ($health -eq "healthy") {
        $ready = $true
        break
    }
    Start-Sleep -Seconds 2
    Write-Host "  ...待機中 ($i/$maxTries) [$health]"
}
if ($ready) {
    Write-Ok "Control DB が healthy になりました"
} else {
    Write-Host "`nControl DB が起動しませんでした。'docker logs control-db' を確認してください。" -ForegroundColor Red
    exit 1
}

# --- 5. フロントエンド依存インストール -------------------------------------
Write-Step "フロントエンド依存をインストール中 (pnpm install)..."
Push-Location frontend
pnpm install
Pop-Location
Write-Ok "フロントエンド依存のインストール完了"

# マイグレーションはバックエンド起動時に sqlx::migrate! で自動適用されるため
# ここでは手動適用は不要。

# --- 完了 ------------------------------------------------------------------
Write-Host "`n=====================================================" -ForegroundColor Green
Write-Host " セットアップ完了！次のコマンドで開発を開始できます:" -ForegroundColor Green
Write-Host "=====================================================" -ForegroundColor Green
Write-Host ""
Write-Host "  # バックエンド (別ターミナル)  ※初回はマイグレーション自動適用"
Write-Host "  cd backend; cargo run"
Write-Host ""
Write-Host "  # フロントエンド (別ターミナル)"
Write-Host "  cd frontend; pnpm dev"
Write-Host ""
Write-Host "  フロント: http://localhost:3000   API: http://localhost:8080"
Write-Host ""
