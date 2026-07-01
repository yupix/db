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

### かんたんセットアップ

前提チェック・`.env` 用意・Control DB 起動・依存インストールを一括で行うスクリプトを用意しています。

```powershell
# Windows (PowerShell)
./scripts/setup.ps1
```

```bash
# Linux / macOS
./scripts/setup.sh
```

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
