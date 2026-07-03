# Neon風 PostgreSQL DB管理サービス

Web上でPostgreSQLインスタンスを作成・管理できるサービス。

## 技術スタック

- **Frontend**: Next.js 14 + TypeScript + Tailwind CSS + shadcn/ui
- **Backend**: Rust + axum + tokio + sqlx
- **Database**: PostgreSQL 16
- **Container**: Docker + bollard
- **Package Manager**: pnpm

## ディレクトリ構成

```
db/
├── frontend/          # Next.js Console
├── backend/           # Rust API Server
├── docker/            # Docker Compose & Dockerfiles
├── tools/
│   └── setup/         # セットアップ CLI (Rust)
└── PLAN.md            # 実装計画
```

## セットアップ

### 前提条件

- Node.js 22+
- pnpm 10+
- Rust 1.85+
- Docker & Docker Compose

### セットアップ CLI のビルド

```bash
cd tools/setup
cargo build --release
```

### 実行

```bash
# 全ステップ一括実行
#   前提確認 → .env 生成 → Control DB 起動 → pnpm install
./tools/setup/target/release/setup

# 個別実行
./tools/setup/target/release/setup check   # 前提コマンド確認のみ
./tools/setup/target/release/setup env     # .env 生成のみ
./tools/setup/target/release/setup db      # Control DB 起動のみ
```

> Windows の場合は `setup.exe` になります。

完了後、別々のターミナルで起動:

```bash
cd backend && cargo run    # API (:8080) ※初回にマイグレーション自動適用
cd frontend && pnpm dev    # Console (:3000)
```

### マイグレーション

マイグレーションはバックエンド起動時に `sqlx::migrate!` で自動適用されるため、
手動での `psql` 実行は不要です (`backend/migrations/` 配下が対象)。

## 進捗

詳細は [PLAN.md](./PLAN.md) を参照。
