#![cfg_attr(not(feature = "console"), windows_subsystem = "windows")]

use anyhow::{Context, Result};
use glzswitch::{config, hotkey, state, websocket};
use std::sync::mpsc::channel;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::time::sleep;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// Global flag for graceful shutdown
static RUNNING: AtomicBool = AtomicBool::new(true);

fn setup_ctrlc_handler() {
    ctrlc::set_handler(|| {
        tracing::info!("Received Ctrl+C, shutting down...");
        RUNNING.store(false, Ordering::SeqCst);
    }).expect("Failed to set Ctrl+C handler");
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

#[tokio::main]
async fn main() -> Result<()> {
    // Setup logging (enable debug to see theme discovery details)
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::new("glzswitch=debug,info"))
        .init();

    tracing::info!("glzswitch starting...");

    // Setup Ctrl+C handler for graceful shutdown
    setup_ctrlc_handler();

    // Initialize paths
    let paths = config::ConfigPaths::new()
        .context("Failed to determine config paths")?;
    
    // Ensure themes directory exists
    config::ensure_themes_dir_exists(&paths)?;
    
    // Create default theme if needed
    config::create_default_theme_if_needed(&paths)?;
    
    // Get theme files
    let themes = config::get_theme_files(&paths)
        .context("Failed to read theme files")?;
    
    if themes.is_empty() {
        anyhow::bail!("No theme files found in {}", paths.theme_dir.display());
    }
    
    tracing::info!("Found {} themes", themes.len());
    for theme in &themes {
        tracing::info!("  - {}", theme.file_name().unwrap_or_default().to_string_lossy());
    }
    
    // Initialize state
    let app_state = state::AppState::new(themes.len());
    
    // Setup hotkey channel
    let (hotkey_tx, hotkey_rx) = channel();
    hotkey::start_hotkey_listener(hotkey_tx)
        .context("Failed to start hotkey listener")?;
    
    // Main loop
    loop {
        // Check for shutdown signal
        if !RUNNING.load(Ordering::SeqCst) {
            tracing::info!("Shutting down...");
            break Ok(());
        }

        // Wait for hotkey with timeout so we can check RUNNING flag
        match hotkey_rx.recv_timeout(Duration::from_millis(500)) {
            Ok(_) => {
                // Check for shutdown signal after receiving hotkey
                if !RUNNING.load(Ordering::SeqCst) {
                    tracing::info!("Shutting down...");
                    break Ok(());
                }
                
                tracing::info!("Hotkey triggered, switching theme...");
                
                // Get next theme index
                let index = app_state.advance();
                let theme = &themes[index];
                
                // Swap theme
                if let Err(e) = config::swap_theme(theme, &paths.glazewm_config) {
                    tracing::error!("Failed to swap theme: {}", e);
                    continue;
                }
                
                // Reload GlazeWM config via WebSocket (with retry)
                match connect_with_retry().await {
                    Ok(mut socket) => {
                        if let Err(e) = socket.reload_config().await {
                            tracing::error!("Failed to reload config: {}", e);
                        } else {
                            tracing::info!("Theme switched to: {}", theme.file_name().unwrap_or_default().to_string_lossy());
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to connect to GlazeWM: {}", e);
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // Timeout is expected - just loop again to check RUNNING flag
                continue;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                tracing::error!("Hotkey channel disconnected");
                break Ok(());
            }
        }
    }
}
