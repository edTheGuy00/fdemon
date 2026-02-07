//! OS signal handling for graceful shutdown

use tokio::sync::mpsc;

use super::message::Message;
use crate::common::prelude::*;

/// Spawn a task that listens for OS signals and sends quit messages
pub fn spawn_signal_handler(tx: mpsc::Sender<Message>) {
    tokio::spawn(async move {
        if let Err(e) = wait_for_signal().await {
            error!("Signal handler error: {}", e);
            return;
        }

        info!("Shutdown signal received");
        let _ = tx.send(Message::Quit).await;
    });
}

/// Wait for a termination signal
async fn wait_for_signal() -> Result<()> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};

        let mut sigint = signal(SignalKind::interrupt())
            .map_err(|e| Error::terminal(format!("Failed to create SIGINT handler: {}", e)))?;
        let mut sigterm = signal(SignalKind::terminate())
            .map_err(|e| Error::terminal(format!("Failed to create SIGTERM handler: {}", e)))?;

        tokio::select! {
            _ = sigint.recv() => {
                info!("Received SIGINT");
            }
            _ = sigterm.recv() => {
                info!("Received SIGTERM");
            }
        }

        Ok(())
    }

    #[cfg(windows)]
    {
        tokio::signal::ctrl_c()
            .await
            .map_err(|e| Error::terminal(format!("Failed to listen for Ctrl+C: {}", e)))?;
        info!("Received Ctrl+C");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_signal_handler_spawn() {
        let (tx, mut rx) = mpsc::channel::<Message>(1);

        // Just verify it spawns without panic
        spawn_signal_handler(tx);

        // Give it a moment to start
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Channel should be empty (no signal sent yet)
        assert!(rx.try_recv().is_err());
    }
}
