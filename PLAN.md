# Neon風 PostgreSQL DB管理サービス - 実装計画

Web上でPostgreSQLインスタンスを作成・管理できるサービス。
開発環境・本番環境を兼ねた、NeonライクなDB管理プラットフォーム。

## 設計決定

| 項目 | 決定 |
|---|---|
| DB Engine | PostgreSQL 16 |
| Frontend | Next.js 14 (App Router) + TypeScript + Tailwind + shadcn/ui |
| SQL Editor | Monaco Editor |
| 状態管理 | TanStack Query + Zustand |
| Charts | Recharts |
| Backend | Rust + axum + tokio + sqlx + bollard |
| DB Driver | sqlx（コンパイル時SQL検証） |
| Docker操作 | bollard（Rust製Docker APIクライアント） |
| 認証 | JWT (jsonwebtoken) + Argon2 |
| Control DB | PostgreSQL 16（メタデータ保存） |
| User DB | PostgreSQL 16（コンテナ・プロジェクト単位） |
| プール | PgBouncer |
| DB分離 | コンテナ/DB（プロジェクト=1 Postgresコンテナ） |
| ブランチ | 作成・リセット・削除のみ（マージなし・Neon準拠） |
| デプロイ | Docker Compose 単一サーバー |

## アーキテクチャ

```
┌──────────────────────────────────────────────────┐
│  Next.js Console (Frontend)                       │
│  Dashboard / Auth / SQL Editor / Metrics / Branch │
└──────────────────┬───────────────────────────────┘
                   │ HTTPS / REST + WebSocket
┌──────────────────▼───────────────────────────────┐
│  Rust Backend (axum)                              │
│  ┌──────┬──────┬───────────┬───────┬──────────┐  │
│  │ Auth │ Proj │Orchestrator│Branch │ Metrics  │  │
│  │      │  API │ (Docker)   │Mgr   │ Collector│  │
│  └──────┴──────┴───────────┴───────┴──────────┘  │
│  ┌──────────┬──────────┐                          │
│  │ Pool Mgr │ Backup   │                          │
│  │(PgBouncer)│Scheduler│                          │
│  └──────────┴──────────┘                          │
└───┬───────────┬───────────────┬──────────────────┘
    │           │               │
┌───▼───┐  ┌────▼─────┐  ┌──────▼──────┐
│Control│  │  User    │  │  PgBouncer  │
│  DB   │  │ Postgres │  │  Containers │
│(meta) │  │ Containers│  │  (pooling)  │
└───────┘  └──────────┘  └─────────────┘
```

## ディレクトリ構成

```
db/
├── frontend/                 # Next.js 14
│   ├── app/
│   │   ├── (auth)/
│   │   │   ├── login/
│   │   │   └── register/
│   │   ├── dashboard/
│   │   ├── projects/
│   │   │   ├── new/
│   │   │   └── [id]/
│   │   ├── layout.tsx
│   │   └── page.tsx
│   ├── components/
│   │   ├── ui/               # shadcn components
│   │   ├── editor/           # Monaco wrapper (Phase 5)
│   │   └── charts/           # Recharts (Phase 7)
│   ├── lib/
│   │   ├── api.ts            # API client
│   │   └── auth.ts
│   ├── middleware.ts         # 認証ガード
│   ├── package.json
│   └── tsconfig.json
├── backend/                  # Rust
│   ├── src/
│   │   ├── main.rs           # axum起動・ルーティング
│   │   ├── config.rs         # 環境変数読込
│   │   ├── state.rs          # AppState (DB pool, Docker)
│   │   ├── error.rs          # 統一エラー型
│   │   ├── api/
│   │   │   ├── mod.rs
│   │   │   ├── auth.rs       # /api/auth/*
│   │   │   ├── projects.rs   # /api/projects/*
│   │   │   ├── branches.rs   # /api/branches/* (Phase 4)
│   │   │   ├── query.rs      # SQL exec (Phase 5)
│   │   │   ├── metrics.rs    # (Phase 7)
│   │   │   ├── backups.rs    # (Phase 8)
│   │   │   └── health.rs
│   │   ├── auth/
│   │   │   ├── mod.rs
│   │   │   ├── jwt.rs
│   │   │   └── password.rs   # Argon2
│   │   ├── orchestrator/
│   │   │   ├── mod.rs
│   │   │   └── docker.rs     # bollard
│   │   ├── branching/        # (Phase 4)
│   │   ├── pool/             # (Phase 3)
│   │   ├── metrics/          # (Phase 7)
│   │   ├── backup/           # (Phase 8)
│   │   └── db/
│   │       ├── mod.rs
│   │       └── models.rs
│   ├── migrations/
│   │   ├── 001_users.sql
│   │   └── 002_projects.sql
│   ├── Cargo.toml
│   └── .env.example
├── docker/
│   ├── docker-compose.yml
│   ├── postgres/
│   │   └── Dockerfile
│   └── pgbouncer/
│       └── Dockerfile
├── .github/workflows/
│   └── ci.yml
├── .gitignore
├── CLAUDE.md
├── PLAN.md
└── README.md
```

---

## Phase 0: プロジェクト基盤 🔧

- [x] 0.1 モノレポ構成作成（frontend/, backend/, docker/, .github/）
- [x] 0.2 Next.js 14 初期化（App Router, TypeScript, Tailwind）
- [x] 0.3 shadcn/ui セットアップ
- [x] 0.4 Rust プロジェクト初期化（axum, sqlx, tokio, bollard, jsonwebtoken, argon2）
- [x] 0.5 Docker Compose 作成（Control DB + Postgresテンプレ + PgBouncer Dockerfile）
- [x] 0.6 Control DB マイグレーション（001_users, 002_projects）
- [x] 0.7 CI設定（cargo fmt/clippy/test + pnpm lint/typecheck/build） ✅ CI通過確認
- [x] 0.8 .gitignore, README 作成
- [ ] 0.9 動作確認: docker compose up → backend起動 → frontend起動

### Control DB スキーマ

```sql
-- 001_users.sql
CREATE TABLE users (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email       TEXT UNIQUE NOT NULL,
    password    TEXT NOT NULL,
    name        TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 002_projects.sql
CREATE TABLE projects (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    slug            TEXT UNIQUE NOT NULL,
    container_id    TEXT,
    container_name  TEXT UNIQUE NOT NULL,
    db_name         TEXT NOT NULL,
    db_user         TEXT NOT NULL,
    db_password     TEXT NOT NULL,
    port            INT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'creating',
    -- creating | running | stopped | error | deleted
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

---

## Phase 1: 認証 & ユーザー管理 🔐

### Rust 側

- [x] 1.1 パスワードハッシュ（Argon2id）実装 (`auth/password.rs`)
- [x] 1.2 JWT発行・検証（access token）実装 (`auth/jwt.rs`)
- [x] 1.3 `POST /api/auth/register` — 登録API
- [x] 1.4 `POST /api/auth/login` — ログインAPI
- [x] 1.5 `GET /api/auth/me` — 現在のユーザー情報
- [x] 1.6 `POST /api/auth/refresh` — リフレッシュトークン
- [x] 1.7 JWT検証ミドルウェア + `AuthUser` extractor
- [x] 1.8 cargo test で認証ロジックのユニットテスト

### Next.js 側

- [x] 1.9 `/login` ページ実装
- [x] 1.10 `/register` ページ実装
- [x] 1.11 httpOnly cookie にJWT保存
- [x] 1.12 `middleware.ts` で未認証ルートガード
- [x] 1.13 `lib/api.ts` fetchラッパー（トークン付与）
- [x] 1.13a 401自動リフレッシュ（refresh実装後に追加）
- [x] 1.14 認証状態フック (`useAuth`)
- [x] 1.15 npm run build 成功確認

---

## Phase 2: プロジェクト & DBインスタンス管理 📦

### Rust 側

- [x] 2.1 `orchestrator/docker.rs` — bollard でPostgresコンテナ起動/停止/削除
- [x] 2.2 `POST /api/projects` — プロジェクト作成（コンテナ起動・接続文字列生成）
- [x] 2.3 `GET /api/projects` — 一覧（自分のプロジェクトのみ）
- [x] 2.4 `GET /api/projects/:id` — 詳細 + 接続情報
- [x] 2.5 `POST /api/projects/:id/start` — 停止中コンテナ起動
- [x] 2.6 `POST /api/projects/:id/stop` — コンテナ停止（データ保持）
- [x] 2.7 `DELETE /api/projects/:id` — コンテナ削除 + レコード削除
- [x] 2.8 `PATCH /api/projects/:id` — 名前変更
- [x] 2.9 空きポート自動割当ロジック
- [x] 2.10 コンテナヘルスチェック待機
- [x] 2.11 cargo test でOrchestrator・プロジェクトAPIのテスト

### Next.js 側

- [x] 2.12 `/dashboard` — プロジェクト一覧カード表示
- [x] 2.13 `/projects/new` — 作成フォーム
- [x] 2.14 `/projects/[id]` — 詳細画面（ステータス・接続文字列・操作ボタン）
- [x] 2.15 TanStack Query でデータフェッチ・キャッシュ
- [x] 2.16 楽観的UI更新（起動/停止）
- [x] 2.17 接続文字列コピーボタン
- [x] 2.18 npm run build 成功確認

### 接続文字列形式

```
直接: postgres://<db_user>:<db_password>@<host>:<port>/<db_name>
プール: postgres://<db_user>:<db_password>@<pooler_host>:<pooler_port>/<db_name>
```

---

## 今後の拡張ロードマップ（後日対応）

### Phase 3: コネクションプール 🔄
- [x] 3.1 プロジェクト単位でPgBouncerコンテナ起動
- [x] 3.2 接続URLをPgBouncer経由にルーティング
- [x] 3.3 プール設定管理API（max_connections, pool_mode等）
- [x] 3.4 環境別エンドポイント（dev/staging/prod）

### Phase 4: ブランチ機能 🌿
- [x] 4.1 ブランチモデル（branches テーブル）
- [x] 4.2 `pg_basebackup` で親から物理コピー → 新コンテナ起動
- [x] 4.3 ブランチリセット（親の指定時点へ戻す）
- [x] 4.4 ブランチリネーム・削除
- [x] 4.5 ブランチツリーUI（親子関係の可視化）

### Phase 5: Web SQLエディタ ⌨️
- [x] 5.1 WebSocketエンドポイント（Rust → ユーザーPostgresへプロキシ）
- [x] 5.2 Monaco Editor統合（SQL構文ハイライト・補完）
- [x] 5.3 クエリ実行・結果テーブル表示
- [x] 5.4 クエリ履歴・保存済みクエリ
- [x] 5.5 実行計画（EXPLAIN）表示

### Phase 6: チーム & 権限 👥
- [ ] 6.1 organizations, teams, members, roles テーブル
- [ ] 6.2 RBAC: owner / admin / developer / viewer
- [ ] 6.3 プロジェクトへのチーム割当
- [ ] 6.4 招待フロー

### Phase 7: メトリクス & モニタリング 📊
- [ ] 7.1 `pg_stat_statements` 拡張でクエリ統計
- [ ] 7.2 Docker stats でコンテナリソース（CPU/メモリ/ディスク/ネットワーク）
- [ ] 7.3 時系列保存（ロールアップ）
- [ ] 7.4 Recharts でグラフ表示
- [ ] 7.5 アラート閾値設定

### Phase 8: バックアップ & リストア 💾
- [ ] 8.1 スケジュール `pg_dump` スナップショット
- [ ] 8.2 PITR: WALアーカイブ + `pg_basebackup`
- [ ] 8.3 スナップショット一覧・復元 UI
- [ ] 8.4 ブランチから復元
- [ ] 8.5 保持ポリシー（日次×7、週次×4 等）

---

## 検証方法

各Phase完了時に以下を実行:

```powershell
# Rust
cd backend; cargo fmt --check; cargo clippy -- -D warnings; cargo test

# Next.js
cd frontend; npm run lint; npm run typecheck; npm run build

# 統合
docker compose up -d
```

## 進捗サマリー

| Phase | 状態 |
|---|---|
| Phase 0: 基盤 | ✅ 完了 |
| Phase 1: 認証 | ✅ 完了 |
| Phase 2: プロジェクト管理 | ✅ 完了 |
| Phase 3: プール | ✅ 完了 |
| Phase 4: ブランチ | ✅ 完了 |
| Phase 5: SQLエディタ | ✅ 完了 |
| Phase 6: チーム | 未開始 |
| Phase 7: メトリクス | 未開始 |
| Phase 8: バックアップ | 未開始 |
