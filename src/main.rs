use std::fs;
use std::path::PathBuf;
use std::process::Command;

use tracing::info;

use hl_evm_rpc::config::Config;
use hl_evm_rpc::hl::HlClient;
use hl_evm_rpc::hl::cache::CachedHlClient;
use hl_evm_rpc::rpc::AppState;

fn pid_file_path() -> PathBuf {
    let dir =
        std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(dir).join("hl-evm-rpc.pid")
}

fn log_file_path() -> PathBuf {
    let dir =
        std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(dir).join("hl-evm-rpc.log")
}

fn read_pid() -> Option<u32> {
    fs::read_to_string(pid_file_path())
        .ok()?
        .trim()
        .parse()
        .ok()
}

fn is_running(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

fn stop_server() {
    if let Some(pid) = read_pid() {
        if is_running(pid) {
            unsafe { libc::kill(pid as i32, libc::SIGTERM) };
            eprintln!("sent SIGTERM to PID {pid}");
            for _ in 0..30 {
                std::thread::sleep(std::time::Duration::from_millis(100));
                if !is_running(pid) {
                    break;
                }
            }
            if is_running(pid) {
                unsafe { libc::kill(pid as i32, libc::SIGKILL) };
                eprintln!("force killed PID {pid}");
            }
        }
        fs::remove_file(pid_file_path()).ok();
        eprintln!("stopped");
    } else {
        eprintln!("not running");
    }
}

fn start_daemon() {
    if let Some(pid) = read_pid() {
        if is_running(pid) {
            eprintln!("already running (PID {pid})");
            std::process::exit(1);
        }
    }

    let exe = std::env::current_exe().expect("cannot get executable path");
    let log_path = log_file_path();
    let log_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .expect("cannot open log file");

    let child = Command::new(exe)
        .arg("serve")
        .env("RUST_LOG", std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(log_file)
        .spawn()
        .expect("failed to spawn daemon");

    eprintln!("started (PID {}), log: {}", child.id(), log_path.display());
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("serve");

    match cmd {
        "stop" => {
            stop_server();
            return;
        }
        "start" => {
            start_daemon();
            return;
        }
        "restart" => {
            stop_server();
            std::thread::sleep(std::time::Duration::from_millis(300));
            start_daemon();
            return;
        }
        "serve" => {}
        other => {
            eprintln!("usage: hl-evm-rpc [serve|start|stop|restart]");
            eprintln!("  serve   - run in foreground (default)");
            eprintln!("  start   - daemonize");
            eprintln!("  stop    - stop running daemon");
            eprintln!("  restart - stop + start");
            if other != "help" && other != "--help" && other != "-h" {
                std::process::exit(1);
            }
            return;
        }
    }

    let config = Config::from_env();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| config.log_level.clone().into()),
        )
        .with_ansi(false)
        .init();

    // Write PID file
    let pid_path = pid_file_path();
    fs::write(&pid_path, std::process::id().to_string()).ok();

    let hl_client = HlClient::new(config.hl_api_url.clone());
    let cached_hl = CachedHlClient::new(hl_client);

    let state = AppState {
        hl: cached_hl,
        chain_id: config.chain_id,
    };

    let app = hl_evm_rpc::build_router(state);

    let listener = tokio::net::TcpListener::bind(&config.listen_addr)
        .await
        .expect("failed to bind");

    info!("listening on {}", config.listen_addr);
    info!("chain_id={} hl_api={}", config.chain_id, config.hl_api_url);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("server error");

    fs::remove_file(&pid_path).ok();
}

async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();
    let mut sigterm =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to register SIGTERM");

    tokio::select! {
        _ = ctrl_c => { info!("received SIGINT, shutting down"); }
        _ = sigterm.recv() => { info!("received SIGTERM, shutting down"); }
    }
}
