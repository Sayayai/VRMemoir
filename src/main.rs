mod api;
mod bio;
mod db;
mod fsm;
mod i18n;
mod recorder;
mod server;
mod session;
mod watcher;

use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::{error, info};

use crate::api::VRChatAPI;
use crate::db::Database;
use crate::fsm::AppFsm;
use crate::recorder::{MicConfig, read_vrchat_mic_device};
use crate::server::AppState;
use crate::watcher::LogWatcher;

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env from the executable's directory
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));

    let env_path = exe_dir.join(".env");
    if !env_path.exists() {
        let example_path = exe_dir.join(".env.example");
        if example_path.exists() {
            info!("{}", t!("env_not_found", ".env.example"));
            if let Err(e) = std::fs::copy(&example_path, &env_path) {
                error!("{}", t!("env_creation_failed", e));
            } else {
                info!("{}", t!("env_created"));
            }
        } else {
            // If .env.example is also missing, create a basic one
            info!("{}", t!("env_not_found", "default template"));
            let default_env = format!("{}", t!("env_template"));
            if let Err(e) = std::fs::write(&env_path, default_env) {
                error!("{}", t!("env_creation_failed", e));
            } else {
                info!("{}", t!("env_created"));
            }
        }
    }

    if env_path.exists() {
        dotenvy::from_path(&env_path).ok();
    } else {
        dotenvy::dotenv().ok();
    }

    // Initialize tracing with local time (no sub-seconds)
    tracing_subscriber::fmt()
        .with_target(false)
        .with_timer(LocalTimer)
        .init();

    // Initialize database (in the same dir as the exe)
    let db_path = exe_dir.join("data.db");
    let db = Arc::new(Database::new(&db_path)?);
    info!("{}", t!("db_initialized", db_path.display()));

    // Initialize API
    let api = Arc::new(VRChatAPI::new(db.clone())?);

    // VRCX-style startup auth: config → check session → auto-login
    info!("{}", t!("authenticating"));
    let auth_result = api.startup_auth().await;
    match auth_result.status {
        api::LoginStatus::Success => info!("{}", t!("auth_success")),
        api::LoginStatus::TwoFactor => {
            // Interactive 2FA prompt at startup
            let tfa_methods = auth_result.requires_two_factor_auth.as_deref().unwrap_or(&[]);
            let method_str = tfa_methods.join(", ");
            println!("\n{}", t!("tfa_required", method_str));
            
            let stdin_2fa = BufReader::new(tokio::io::stdin());
            let mut lines_2fa = stdin_2fa.lines();

            loop {
                println!("{}", t!("tfa_prompt"));
                if let Ok(Some(code_line)) = lines_2fa.next_line().await {
                    let code = code_line.trim().to_string();
                    if code.is_empty() {
                        info!("{}", t!("tfa_skipped"));
                        break;
                    }

                    // Determine 2FA type from the methods array
                    let tfa_type = if tfa_methods.contains(&"emailOtp".to_string()) {
                        "emailotp"
                    } else if tfa_methods.contains(&"totp".to_string()) {
                        "totp"
                    } else if tfa_methods.contains(&"otp".to_string()) {
                        "otp"
                    } else {
                        "emailotp" // default
                    };

                    println!("{}", t!("tfa_verifying", tfa_type));
                    let tfa_result = api.verify_2fa(tfa_type, &code).await;
                    match tfa_result.status {
                        api::LoginStatus::Success => {
                            info!("{}", t!("tfa_success"));
                            break;
                        }
                        _ => {
                            println!("{}", t!("tfa_failed", 
                                tfa_result.message.as_deref().unwrap_or("unknown error")));
                            println!("{}", t!("tfa_retry"));
                        }
                    }
                } else {
                    break;
                }
            }
        }
        api::LoginStatus::Failed => info!("{}", t!("auth_failed",
            auth_result.message.as_deref().unwrap_or("Please login via API"))),
    }



    // Keep-alive task (every 5 minutes)
    let api_keepalive = api.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
        interval.tick().await; // Skip first immediate tick
        loop {
            interval.tick().await;
            api_keepalive.keep_alive().await;
        }
    });

    // --- Mic recording config ---
    let record_mic = std::env::var("RECORD_MIC")
        .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
        .unwrap_or(false);
    let mic_device_env = std::env::var("MIC_DEVICE").ok().filter(|s| !s.trim().is_empty());

    let mic_device_name = if record_mic {
        // Priority: env var > VRChat registry > system default (None)
        if let Some(ref dev) = mic_device_env {
            info!("{}", t!("mic_device_env", dev));
            Some(dev.clone())
        } else {
            let reg_dev = read_vrchat_mic_device();
            if reg_dev.is_some() {
                info!("{}", t!("mic_device_reg"));
            } else {
                info!("{}", t!("mic_device_default"));
            }
            reg_dev
        }
    } else {
        None
    };

    let mic_config = MicConfig {
        enabled: record_mic,
        device_name: mic_device_name,
    };
    info!("{}", t!("mic_recording_status", if record_mic { t!("enabled") } else { t!("disabled") }));

    // Start log watcher
    let log_watcher = LogWatcher::new();
    let mut rx = log_watcher.start().await?;

    // Initialize BIO Manager
    let bio_manager = Arc::new(crate::bio::BioManager::new(api.clone(), db.clone()));

    // 创建有限状态机 (FSM)
    let mic_config_shared = Arc::new(mic_config);
    let fsm = Arc::new(tokio::sync::Mutex::new(
        AppFsm::new(
            db.clone(), 
            bio_manager.clone(),
            mic_config_shared, 
            exe_dir.clone(),
        ),
    ));

    // 事件处理循环 — 所有状态转换由 FSM 统一管理
    let fsm_for_events = fsm.clone();
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            let mut fsm = fsm_for_events.lock().await;
            fsm.handle_event(event);
        }
    });

    // 进程退出检测 — 轮询 VRChat 进程是否存活
    let fsm_for_monitor = fsm.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
        loop {
            interval.tick().await;
            let mut fsm = fsm_for_monitor.lock().await;
            fsm.check_process_alive();
        }
    });


    // Start HTTP server
    let app_state = Arc::new(AppState {
        db: db.clone(),
        api: api.clone(),
        bio: bio_manager.clone(),
    });

    let router = server::create_router(app_state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3001").await?;
    info!("{}", t!("api_server_started"));

    // Spawn the server
    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, router).await {
            error!("{}", t!("server_error", e));
        }
    });

    // Wait for Ctrl+C to shutdown gracefully
    info!("{}", t!("watcher_started"));
    tokio::signal::ctrl_c().await?;

    // 退出时安全结束录音
    {
        let mut fsm = fsm.lock().await;
        fsm.shutdown();
    }
    info!("{}", t!("exited"));

    Ok(())
}

/// Custom timer that formats timestamps in local time without sub-seconds.
/// Output format: `2026-02-13 13:24:00`
struct LocalTimer;

impl tracing_subscriber::fmt::time::FormatTime for LocalTimer {
    fn format_time(&self, w: &mut tracing_subscriber::fmt::format::Writer<'_>) -> std::fmt::Result {
        let now = chrono::Local::now();
        write!(w, "{}", now.format("%Y-%m-%d %H:%M:%S"))
    }
}
