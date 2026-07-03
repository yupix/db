use clap::{Parser, Subcommand};
use console::{style, Term};
use dialoguer::{Input, Password};
use rand::Rng;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::Duration,
};

#[derive(Parser)]
#[command(name = "setup", about = "開発環境セットアップツール", version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// リポジトリルートのパス (省略時: このバイナリの2階層上)
    #[arg(long, global = true)]
    root: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// 前提コマンドの確認のみ行う
    Check,
    /// .env ファイルの生成のみ行う
    Env,
    /// Control DB の起動と healthy 待ちのみ行う
    Db,
}

fn main() {
    let cli = Cli::parse();
    let term = Term::stdout();

    let root = cli
        .root
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let root = root.canonicalize().unwrap_or(root);

    match cli.command {
        Some(Commands::Check) => {
            step(&term, "前提コマンドを確認中...");
            check_prereqs();
        }
        Some(Commands::Env) => {
            step(&term, "環境変数ファイルを生成中...");
            gen_env(&root);
        }
        Some(Commands::Db) => {
            step(&term, "Control DB を起動中...");
            start_db(&root);
            wait_healthy(&term);
        }
        None => {
            run_all(&term, &root);
        }
    }
}

fn run_all(term: &Term, root: &Path) {
    step(term, "前提コマンドを確認中...");
    check_prereqs();

    step(term, "環境変数ファイルを生成中...");
    gen_env(root);

    step(term, "Control DB (PostgreSQL) を起動中...");
    start_db(root);
    wait_healthy(term);

    step(term, "フロントエンド依存をインストール中 (pnpm install)...");
    pnpm_install(root);

    println!();
    println!("{}", style("=====================================================").green().bold());
    println!("{}", style(" セットアップ完了！次のコマンドで開発を開始できます:").green().bold());
    println!("{}", style("=====================================================").green().bold());
    println!();
    println!("  # バックエンド (別ターミナル)  ※初回はマイグレーション自動適用");
    println!("  cd backend && cargo run");
    println!();
    println!("  # フロントエンド (別ターミナル)");
    println!("  cd frontend && pnpm dev");
    println!();
    println!("  フロント: http://localhost:3000   API: http://localhost:8080");
    println!();
}

// ---------------------------------------------------------------------------
// ステップ出力
// ---------------------------------------------------------------------------

fn step(term: &Term, msg: &str) {
    let _ = term.write_line(&format!("\n{} {}", style("==>").cyan().bold(), style(msg).bold()));
}

fn ok(msg: &str) {
    println!("  {}  {}", style("OK").green().bold(), msg);
}

fn warn(msg: &str) {
    println!("  {}  {}", style("!!").yellow().bold(), msg);
}

// ---------------------------------------------------------------------------
// 前提確認
// ---------------------------------------------------------------------------

fn check_prereqs() {
    let tools = ["docker", "cargo", "pnpm", "node"];
    let mut missing = vec![];

    for tool in tools {
        if which(tool) {
            ok(&format!("{tool} が見つかりました"));
        } else {
            warn(&format!("{tool} が見つかりません"));
            missing.push(tool);
        }
    }

    if !missing.is_empty() {
        eprintln!(
            "\n{} 以下をインストールしてください: {}",
            style("ERROR").red().bold(),
            missing.join(", ")
        );
        eprintln!("  docker : https://docs.docker.com/get-docker/");
        eprintln!("  rust   : https://rustup.rs/");
        eprintln!("  pnpm   : https://pnpm.io/installation (Node.js 22+ 同梱)");
        std::process::exit(1);
    }

    let docker_ok = Command::new("docker")
        .args(["info"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !docker_ok {
        eprintln!(
            "\n{} Docker デーモンが起動していません。Docker を起動してください。",
            style("ERROR").red().bold()
        );
        std::process::exit(1);
    }
    ok("Docker デーモンが稼働中");
}

fn which(cmd: &str) -> bool {
    let out = if cfg!(windows) {
        Command::new("where").arg(cmd).output()
    } else {
        Command::new("which").arg(cmd).output()
    };
    out.map(|o| o.status.success()).unwrap_or(false)
}

// ---------------------------------------------------------------------------
// .env 生成
// ---------------------------------------------------------------------------

fn gen_env(root: &Path) {
    gen_backend_env(root);
    gen_frontend_env(root);
}

fn gen_backend_env(root: &Path) {
    let env_path = root.join("backend").join(".env");

    if env_path.exists() {
        ok("backend/.env は既に存在します (スキップ — 変更したい場合は削除してから再実行)");
        return;
    }

    println!();
    println!("  DB / サーバー設定を入力してください。Enter でデフォルト値を使用します。");
    println!();

    let db_host: String = prompt("Control DB ホスト", "localhost");
    let db_port: String = prompt("Control DB ポート", "5432");
    let db_name: String = prompt("Control DB 名", "dbcontrol");
    let db_user: String = prompt("DB ユーザー", "admin");
    let db_pass: String = prompt_password("DB パスワード", "admin123");
    let api_port: String = prompt("バックエンドポート", "8080");
    let backup_dir: String = prompt("バックアップ保存先", "./data/backups");

    let auto_secret = random_secret();
    let jwt_secret: String = prompt("JWT シークレット (Enter で自動生成)", &auto_secret);

    let content = format!(
        "# Database URL for Control DB\n\
         DATABASE_URL=postgres://{db_user}:{db_pass}@{db_host}:{db_port}/{db_name}\n\
         \n\
         # JWT Secret\n\
         JWT_SECRET={jwt_secret}\n\
         \n\
         # Server\n\
         HOST=0.0.0.0\n\
         PORT={api_port}\n\
         \n\
         # Logging\n\
         RUST_LOG=info,backend=debug\n\
         \n\
         # Host-side directory backup archives are written to\n\
         BACKUP_DIR={backup_dir}\n"
    );

    fs::write(&env_path, content).unwrap_or_else(|e| {
        eprintln!("backend/.env の書き込みに失敗: {e}");
        std::process::exit(1);
    });
    ok("backend/.env を生成しました");

    if db_user != "admin" || db_pass != "admin123" || db_name != "dbcontrol" {
        warn("docker/docker-compose.yml の DB 設定と異なる値を入力しました。");
        warn("docker-compose.yml 側も合わせて編集してください。");
    }
}

fn gen_frontend_env(root: &Path) {
    let env_path = root.join("frontend").join(".env.local");

    if env_path.exists() {
        ok("frontend/.env.local は既に存在します (スキップ)");
        return;
    }

    let backend_env = root.join("backend").join(".env");
    let default_port = if backend_env.exists() {
        fs::read_to_string(&backend_env)
            .unwrap_or_default()
            .lines()
            .find(|l| l.starts_with("PORT="))
            .and_then(|l| l.strip_prefix("PORT="))
            .map(|v| v.trim().to_string())
            .unwrap_or_else(|| "8080".to_string())
    } else {
        "8080".to_string()
    };

    let default_url = format!("http://localhost:{default_port}");
    let api_url: String = prompt("フロント → バックエンド URL", &default_url);

    fs::write(&env_path, format!("NEXT_PUBLIC_API_URL={api_url}\n")).unwrap_or_else(|e| {
        eprintln!("frontend/.env.local の書き込みに失敗: {e}");
        std::process::exit(1);
    });
    ok("frontend/.env.local を生成しました");
}

// ---------------------------------------------------------------------------
// Docker
// ---------------------------------------------------------------------------

fn start_db(root: &Path) {
    let compose = root.join("docker").join("docker-compose.yml");
    let status = Command::new("docker")
        .args(["compose", "-f", compose.to_str().unwrap(), "up", "control-db", "-d"])
        .status()
        .unwrap_or_else(|e| {
            eprintln!("docker compose 起動に失敗: {e}");
            std::process::exit(1);
        });
    if !status.success() {
        eprintln!("{} docker compose up が失敗しました", style("ERROR").red().bold());
        std::process::exit(1);
    }
    ok("control-db コンテナを起動しました");
}

fn wait_healthy(term: &Term) {
    step(term, "Control DB の起動を待機中...");
    for i in 1..=30 {
        let health = Command::new("docker")
            .args(["inspect", "--format", "{{.State.Health.Status}}", "control-db"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|_| "none".to_string());

        if health == "healthy" {
            ok("Control DB が healthy になりました");
            return;
        }
        println!("  ...待機中 ({i}/30) [{health}]");
        thread::sleep(Duration::from_secs(2));
    }
    eprintln!(
        "\n{} Control DB が起動しませんでした。'docker logs control-db' を確認してください。",
        style("ERROR").red().bold()
    );
    std::process::exit(1);
}

// ---------------------------------------------------------------------------
// pnpm install
// ---------------------------------------------------------------------------

fn pnpm_install(root: &Path) {
    let status = Command::new("pnpm")
        .arg("install")
        .current_dir(root.join("frontend"))
        .status()
        .unwrap_or_else(|e| {
            eprintln!("pnpm install に失敗: {e}");
            std::process::exit(1);
        });
    if !status.success() {
        eprintln!("{} pnpm install が失敗しました", style("ERROR").red().bold());
        std::process::exit(1);
    }
    ok("フロントエンド依存のインストール完了");
}

// ---------------------------------------------------------------------------
// ユーティリティ
// ---------------------------------------------------------------------------

fn prompt(label: &str, default: &str) -> String {
    Input::new()
        .with_prompt(format!("  {label}"))
        .default(default.to_string())
        .interact_text()
        .unwrap_or_else(|_| default.to_string())
}

fn prompt_password(label: &str, default: &str) -> String {
    Password::new()
        .with_prompt(format!("  {label}"))
        .allow_empty_password(true)
        .interact()
        .unwrap_or_else(|_| default.to_string())
}

fn random_secret() -> String {
    let mut rng = rand::thread_rng();
    let charset: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"
        .chars()
        .collect();
    (0..48).map(|_| charset[rng.gen_range(0..charset.len())]).collect()
}
