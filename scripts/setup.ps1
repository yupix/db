#!/usr/bin/env pwsh
# ---------------------------------------------------------------------------
# 開発環境セットアップスクリプト (Windows / PowerShell)
#
#   前提チェック → .env 用意 → Control DB 起動 → healthy 待ち →
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

# docker デーモンが動いているか
try {
    docker info *> $null
    Write-Ok "Docker デーモンが稼働中"
} catch {
    Write-Host "`nDocker デーモンが起動していません。Docker Desktop を起動してください。" -ForegroundColor Red
    exit 1
}

# --- 2. backend/.env の用意 ------------------------------------------------
Write-Step "backend/.env を用意中..."
if (Test-Path "backend/.env") {
    Write-Ok "backend/.env は既に存在します (スキップ)"
} else {
    Copy-Item "backend/.env.example" "backend/.env"
    Write-Ok "backend/.env.example から backend/.env を作成しました"
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
