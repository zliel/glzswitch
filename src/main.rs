#![cfg_attr(not(feature = "console"), windows_subsystem = "windows")]

use anyhow::{Context, Result};
use glzswitch::{config, hotkey, state, websocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::time::Duration;
use tokio::task::spawn;
use tokio::time::sleep;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

static RUNNING: AtomicBool = AtomicBool::new(true);

fn setup_ctrlc_handler() {
    ctrlc::set_handler(|| {
        tracing::info!("Received Ctrl+C, shutting down...");
        RUNNING.store(false, Ordering::SeqCst);
    })
    .expect("Failed to set Ctrl+C handler");
}

async fn connect_with_retry() -> anyhow::Result<websocket::GlazeSocket> {
    let mut attempts = 0;
    let max_attempts = 3;

    loop {
        match websocket::GlazeSocket::connect().await {
            Ok(socket) => return Ok(socket),
            Err(e) if attempts < max_attempts - 1 => {
                attempts += 1;
                tracing::warn!("Connection attempt {} failed: {}", attempts, e);
                sleep(Duration::from_secs(1)).await;
            }
            Err(e) => return Err(e),
        }
    }
}

async fn monitor_glazewm_connection() {
    loop {
        if !RUNNING.load(Ordering::SeqCst) {
            break;
        }

        match websocket::GlazeSocket::connect().await {
            Ok(mut socket) => {
                tracing::info!("Monitoring GlazeWM connection...");
                socket.wait_for_close().await;
                tracing::info!("GlazeWM has exited, shutting down glzswitch...");
                RUNNING.store(false, Ordering::SeqCst);
            }
            Err(e) => {
                tracing::warn!("Failed to connect to GlazeWM for monitoring: {}", e);
                sleep(Duration::from_secs(2)).await;
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::new("glzswitch=debug,info"))
        .init();

    tracing::info!("glzswitch starting...");

    setup_ctrlc_handler();

    let paths = config::ConfigPaths::new().context("Failed to determine config paths")?;

    config::ensure_themes_dir_exists(&paths)?;
    config::create_default_theme_if_needed(&paths)?;

    let themes = config::get_themes(&paths).context("Failed to read theme files")?;

    if themes.is_empty() {
        anyhow::bail!("No theme files found in {}", paths.theme_dir.display());
    }

    tracing::info!("Found {} themes", themes.len());
    for theme in &themes {
        tracing::info!(
            "  - {}",
            theme.file_name().unwrap_or_default().to_string_lossy()
        );
    }

    let app_state = state::AppState::new(themes.len());

    let (hotkey_tx, hotkey_rx) = channel();
    hotkey::start_hotkey_listener(hotkey_tx).context("Failed to start hotkey listener")?;

    let glazewm_monitor = spawn(monitor_glazewm_connection());

    loop {
        if !RUNNING.load(Ordering::SeqCst) {
            tracing::info!("Shutting down...");
            break Ok(());
        }

        match hotkey_rx.recv_timeout(Duration::from_millis(500)) {
            Ok(_) => {
                if !RUNNING.load(Ordering::SeqCst) {
                    tracing::info!("Shutting down...");
                    break Ok(());
                }

                tracing::info!("Hotkey triggered, switching theme...");

                const COOLDOWN: Duration = Duration::from_millis(1500);
                if app_state.is_in_cooldown(COOLDOWN) {
                    tracing::info!("Theme switch on cooldown, skipping...");
                    continue;
                }

                let index = app_state.advance();
                let theme = &themes[index];

                if let Err(e) = config::swap_theme(theme, &paths) {
                    tracing::error!("Failed to swap theme: {}", e);
                    continue;
                }

                match connect_with_retry().await {
                    Ok(mut socket) => match socket.reload_config_with_event(2).await {
                        Ok(_) => {
                            app_state.record_switch();
                            tracing::info!(
                                "Theme switched to: {}",
                                theme.file_name().unwrap_or_default().to_string_lossy()
                            );
                        }
                        Err(e) => {
                            app_state.record_switch();
                            tracing::warn!("Config reload event: {}", e);
                            tracing::info!(
                                "Theme applied: {}",
                                theme.file_name().unwrap_or_default().to_string_lossy()
                            );
                        }
                    },
                    Err(e) => {
                        tracing::error!("Failed to connect to GlazeWM: {}", e);
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                continue;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                tracing::error!("Hotkey channel disconnected");
                break Ok(());
            }
        }
    }
}
