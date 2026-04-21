//! WebSocket client for GlazeWM IPC communication.
//!
//! This module provides a WebSocket client to connect to GlazeWM's built-in
//! WebSocket server and send commands like `wm-reload-config`.

use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use http::Uri;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async,
    tungstenite::Message,
    WebSocketStream,
    MaybeTlsStream,
};

/// Default WebSocket URL for GlazeWM.
const GLAZEWM_WS_URL: &str = "ws://localhost:6123";

/// GlazeWM WebSocket client for sending commands.
///
/// This client connects to GlazeWM's WebSocket server and provides methods
/// to send commands. Only write operations are supported (sending commands).
pub struct GlazeSocket {
    write: futures_util::stream::SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
    read: Option<futures_util::stream::SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>>,
}

impl GlazeSocket {
    /// Subscribe to events and wait for user_config_changed.
    ///
    /// This sends a subscribe command for user_config_changed and waits for the event.
    /// Returns Ok(()) if the event is received, Err if timeout.
    pub async fn wait_for_config_reload(&mut self, timeout_secs: u64) -> Result<()> {
        let msg = Message::Text("subscribe user_config_changed".to_string());
        self.write
            .send(msg)
            .await
            .context("Failed to subscribe to user_config_changed")?;

        let timeout = std::time::Duration::from_secs(timeout_secs);

        if let Some(read) = self.read.take() {
            let read = read;
            tokio::pin!(read);

            loop {
                tokio::select! {
                    result = tokio::time::timeout(timeout, read.next()) => {
                        match result {
                            Ok(Some(Ok(Message::Text(text)))) => {
                                if text.contains("user_config_changed") {
                                    tracing::info!("Received user_config_changed event");
                                    return Ok(());
                                }
                            }
                            Ok(Some(Ok(Message::Close(_)))) | Ok(Some(Err(_))) | Ok(None) => {
                                break;
                            }
                            Err(_) => {
                                break;
                            }
                            _ => {}
                        }
                    }
                    _ = tokio::time::sleep(timeout) => {
                        break;
                    }
                }
            }
        }

        anyhow::bail!("Timeout waiting for user_config_changed event");
    }
}

impl GlazeSocket {
    /// Connect to GlazeWM's WebSocket server.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection fails or the URL is invalid.
    pub async fn connect() -> Result<Self> {
        let url: Uri = GLAZEWM_WS_URL
            .parse()
            .context("Failed to parse GlazeWM WS URL")?;

        let (ws_stream, _) = connect_async(url)
            .await
            .context("Failed to connect to GlazeWM WebSocket")?;

        let (write, read) = ws_stream.split();

        tracing::info!("Connected to GlazeWM WebSocket at {}", GLAZEWM_WS_URL);

        Ok(GlazeSocket { write, read: Some(read) })
    }

    /// Connect to a custom GlazeWM WebSocket URL.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection fails or the URL is invalid.
    pub async fn connect_to(url: &str) -> Result<Self> {
        let parsed_url: Uri = url
            .parse()
            .context("Failed to parse GlazeWM WS URL")?;

        let (ws_stream, _) = connect_async(parsed_url)
            .await
            .context("Failed to connect to GlazeWM WebSocket")?;

        let (write, read) = ws_stream.split();

        tracing::info!("Connected to GlazeWM WebSocket at {}", url);

        Ok(GlazeSocket { write, read: Some(read) })
    }

    /// Send a command to GlazeWM.
    ///
    /// Commands are sent as text messages prefixed with "command ".
    /// For example, to reload the config, send "command wm-reload-config".
    ///
    /// # Errors
    ///
    /// Returns an error if sending the command fails.
    pub async fn send_command(&mut self, command: &str) -> Result<()> {
        let msg = Message::Text(format!("command {}", command));
        self.write
            .send(msg)
            .await
            .context("Failed to send command")?;
        tracing::debug!("Sent command: {}", command);
        Ok(())
    }

/// Reload GlazeWM's configuration.
///
/// This is a convenience method that sends the `wm-reload-config` command.
///
/// # Errors
///
/// Returns an error if sending the command fails.
pub async fn reload_config(&mut self) -> Result<()> {
    self.send_command("wm-reload-config").await
}

/// Reload config and wait for the event.
///
/// Subscribes to user_config_changed first, then sends reload command,
/// and waits for the event to confirm reload completed.
///
/// # Errors
///
/// Returns an error if any step fails.
pub async fn reload_config_with_event(&mut self, timeout_secs: u64) -> Result<()> {
    // Subscribe first so we don't miss the event
    let msg = Message::Text("subscribe user_config_changed".to_string());
    self.write
        .send(msg)
        .await
        .context("Failed to subscribe to user_config_changed")?;

    // Small delay to ensure subscription is registered
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Send reload command
    self.send_command("wm-reload-config").await?;

    // Wait for the event
    let timeout = std::time::Duration::from_secs(timeout_secs);

    if let Some(read) = self.read.take() {
        let read = read;
        tokio::pin!(read);

        loop {
            tokio::select! {
                result = tokio::time::timeout(timeout, read.next()) => {
                    match result {
                        Ok(Some(Ok(Message::Text(text)))) => {
                            if text.contains("user_config_changed") {
                                tracing::info!("Received user_config_changed event");
                                return Ok(());
                            }
                        }
                        Ok(Some(Ok(Message::Close(_)))) | Ok(Some(Err(_))) | Ok(None) => {
                            break;
                        }
                        Err(_) => {
                            break;
                        }
                        _ => {}
                    }
                }
                _ = tokio::time::sleep(timeout) => {
                    break;
                }
            }
        }
    }

    anyhow::bail!("Timeout waiting for user_config_changed event");
}

    /// Close the WebSocket connection gracefully.
    ///
    /// # Errors
    ///
    /// Returns an error if closing the connection fails.
    pub async fn close(&mut self) -> Result<()> {
        self.write
            .send(Message::Close(None))
            .await
            .context("Failed to close WebSocket connection")?;
        tracing::info!("Closed GlazeWM WebSocket connection");
        Ok(())
    }

    /// Wait for the connection to be closed (i.e., GlazeWM has exited).
    ///
    /// This returns a future that completes when the WebSocket connection is closed,
    /// which happens when GlazeWM exits.
    pub async fn wait_for_close(&mut self) {
        if let Some(read) = self.read.take() {
            let mut read = read;
            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Close(_)) | Err(_) => {
                        tracing::info!("GlazeWM connection closed");
                        break;
                    }
                    Ok(Message::Ping(data)) => {
                        let _ = self.write.send(Message::Pong(data)).await;
                    }
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glaze_ws_url_constant() {
        assert_eq!(GLAZEWM_WS_URL, "ws://localhost:6123");
    }
}
