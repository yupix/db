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
└── PLAN.md            # 実装計画
```

## セットアップ

### 前提条件
- Node.js 22+
- pnpm 10+
- Rust 1.85+
- Docker & Docker Compose

### 開発環境起動

```bash
# 1. Control DB起動
cd docker
docker compose up control-db -d

# 2. バックエンド起動
cd backend
cp .env.example .env
cargo run

# 3. フロントエンド起動
cd frontend
pnpm dev
```

### マイグレーション

```bash
# Control DBにマイグレーション適用
psql postgres://admin:admin123@localhost:5432/dbcontrol -f backend/migrations/001_users.sql
psql postgres://admin:admin123@localhost:5432/dbcontrol -f backend/migrations/002_projects.sql
```

## 進捗

詳細は [PLAN.md](./PLAN.md) を参照。
