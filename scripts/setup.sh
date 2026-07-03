#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# 開発環境セットアップスクリプト (Linux / macOS)
#
#   前提チェック → .env 対話生成 → Control DB 起動 → healthy 待ち →
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

# デフォルト値付き入力。空 Enter でデフォルトを採用。
ask() {
    local prompt="$1" default="$2" reply
    read -rp "  $prompt [$default]: " reply
    echo "${reply:-$default}"
}

# ランダム英数字 48文字
random_secret() {
    set +o pipefail
    LC_ALL=C tr -dc 'A-Za-z0-9' < /dev/urandom | head -c 48
    set -o pipefail
}

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

# --- 2. backend/.env の生成 ------------------------------------------------
cyan "backend/.env を設定中..."
if [ -f backend/.env ]; then
    ok "backend/.env は既に存在します (スキップ — 変更したい場合は削除してから再実行)"
else
    echo ""
    echo "  DB / サーバー設定を入力してください。Enter でデフォルト値を使用します。"
    echo ""

    db_host=$(ask "Control DB ホスト"     "localhost")
    db_port=$(ask "Control DB ポート"     "5432")
    db_name=$(ask "Control DB 名"         "dbcontrol")
    db_user=$(ask "DB ユーザー"           "admin")
    db_pass=$(ask "DB パスワード"         "admin123")
    api_port=$(ask "バックエンドポート"   "8080")
    backup_dir=$(ask "バックアップ保存先" "./data/backups")

    auto_secret="$(random_secret)"
    jwt_secret=$(ask "JWT シークレット (Enter で自動生成)" "$auto_secret")

    cat > backend/.env <<EOF
# Database URL for Control DB
DATABASE_URL=postgres://${db_user}:${db_pass}@${db_host}:${db_port}/${db_name}

# JWT Secret
JWT_SECRET=${jwt_secret}

# Server
HOST=0.0.0.0
PORT=${api_port}

# Logging
RUST_LOG=info,backend=debug

# Host-side directory backup archives are written to
BACKUP_DIR=${backup_dir}
EOF

    ok "backend/.env を生成しました"

    # docker-compose の DB 設定と異なる場合は警告
    if [ "$db_user" != "admin" ] || [ "$db_pass" != "admin123" ] || [ "$db_name" != "dbcontrol" ]; then
        warn "docker/docker-compose.yml の DB 設定と異なる値を入力しました。"
        warn "docker-compose.yml 側も合わせて編集してください。"
    fi
fi

# frontend/.env.local の生成
echo ""
if [ -f frontend/.env.local ]; then
    ok "frontend/.env.local は既に存在します (スキップ)"
else
    # backend/.env から PORT を読む
    api_port_from_env=""
    if [ -f backend/.env ]; then
        api_port_from_env="$(grep -E '^PORT=' backend/.env | cut -d= -f2 | tr -d '[:space:]')"
    fi
    api_port_from_env="${api_port_from_env:-8080}"

    api_url=$(ask "フロント → バックエンド URL" "http://localhost:${api_port_from_env}")
    echo "NEXT_PUBLIC_API_URL=${api_url}" > frontend/.env.local
    ok "frontend/.env.local を生成しました"
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
