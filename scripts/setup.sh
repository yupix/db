#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# 開発環境セットアップスクリプト (Linux / macOS)
#
#   前提チェック → .env 用意 → Control DB 起動 → healthy 待ち →
#   フロントエンド依存インストール まで一括で行う。
#
# 使い方:  ./scripts/setup.sh
# ---------------------------------------------------------------------------
set -euo pipefail

# リポジトリルート (このスクリプトの1つ上) へ移動
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

cyan()  { printf '\n\033[36m==> %s\033[0m\n' "$1"; }
ok()    { printf '\033[32m  OK  %s\033[0m\n' "$1"; }
warn()  { printf '\033[33m  !!  %s\033[0m\n' "$1"; }
fail()  { printf '\033[31m%s\033[0m\n' "$1"; }

# --- 1. 前提コマンドの確認 -------------------------------------------------
cyan "前提コマンドを確認中..."
missing=()
for cmd in docker cargo pnpm node; do
    if command -v "$cmd" >/dev/null 2>&1; then
        ok "$cmd が見つかりました"
    else
        missing+=("$cmd")
        warn "$cmd が見つかりません"
    fi
done
if [ ${#missing[@]} -gt 0 ]; then
    fail "\n以下のコマンドをインストールしてください: ${missing[*]}"
    echo "  docker: https://docs.docker.com/get-docker/"
    echo "  rust  : https://rustup.rs/"
    echo "  pnpm  : https://pnpm.io/installation (Node.js 22+ 同梱)"
    exit 1
fi

if ! docker info >/dev/null 2>&1; then
    fail "\nDocker デーモンが起動していません。Docker を起動してください。"
    exit 1
fi
ok "Docker デーモンが稼働中"

# --- 2. backend/.env の用意 ------------------------------------------------
cyan "backend/.env を用意中..."
if [ -f backend/.env ]; then
    ok "backend/.env は既に存在します (スキップ)"
else
    cp backend/.env.example backend/.env
    ok "backend/.env.example から backend/.env を作成しました"
fi

# --- 3. Control DB を起動 --------------------------------------------------
cyan "Control DB (PostgreSQL) を起動中..."
docker compose -f docker/docker-compose.yml up control-db -d
ok "control-db コンテナを起動しました"

# --- 4. healthy 待ち -------------------------------------------------------
cyan "Control DB の起動を待機中..."
ready=false
for i in $(seq 1 30); do
    health="$(docker inspect --format '{{.State.Health.Status}}' control-db 2>/dev/null || echo none)"
    if [ "$health" = "healthy" ]; then
        ready=true
        break
    fi
    sleep 2
    echo "  ...待機中 ($i/30) [$health]"
done
if [ "$ready" = true ]; then
    ok "Control DB が healthy になりました"
else
    fail "\nControl DB が起動しませんでした。'docker logs control-db' を確認してください。"
    exit 1
fi

# --- 5. フロントエンド依存インストール -------------------------------------
cyan "フロントエンド依存をインストール中 (pnpm install)..."
( cd frontend && pnpm install )
ok "フロントエンド依存のインストール完了"

# マイグレーションはバックエンド起動時に sqlx::migrate! で自動適用されるため
# ここでは手動適用は不要。

# --- 完了 ------------------------------------------------------------------
printf '\n\033[32m=====================================================\n'
printf ' セットアップ完了！次のコマンドで開発を開始できます:\n'
printf '=====================================================\033[0m\n\n'
echo "  # バックエンド (別ターミナル)  ※初回はマイグレーション自動適用"
echo "  cd backend && cargo run"
echo ""
echo "  # フロントエンド (別ターミナル)"
echo "  cd frontend && pnpm dev"
echo ""
echo "  フロント: http://localhost:3000   API: http://localhost:8080"
echo ""
