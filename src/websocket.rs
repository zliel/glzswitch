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
