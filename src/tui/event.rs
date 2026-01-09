//! Terminal event polling

use crate::app::message::Message;
use crate::common::prelude::*;
use crossterm::event::{self, Event, KeyCode};
use std::time::Duration;

/// Poll for terminal events with timeout
pub fn poll() -> Result<Option<Message>> {
    // Poll with 50ms timeout (20 FPS)
    if event::poll(Duration::from_millis(50))? {
        let event = event::read()?;

        // Temporary debug logging to investigate PTY key event handling
        tracing::debug!("Raw crossterm event: {:?}", event);

        match event {
            Event::Key(key) => {
                tracing::debug!(
                    "Key event: code={:?}, kind={:?}, modifiers={:?}",
                    key.code,
                    key.kind,
                    key.modifiers
                );

                // Special logging for Enter and Space keys (the problematic keys in PTY)
                if matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) {
                    tracing::warn!(
                        "ENTER/SPACE KEY DETECTED: code={:?}, kind={:?}, modifiers={:?}",
                        key.code,
                        key.kind,
                        key.modifiers
                    );
                }

                if key.kind == event::KeyEventKind::Press {
                    tracing::debug!("Accepting KeyEventKind::Press - forwarding to handler");
                    Ok(Some(Message::Key(key)))
                } else {
                    tracing::debug!("Ignoring non-Press key event (kind={:?})", key.kind);
                    Ok(None)
                }
            }
            _ => {
                tracing::debug!("Non-key event: {:?}", event);
                Ok(None)
            }
        }
    } else {
        // Generate tick on timeout for animations
        Ok(Some(Message::Tick))
    }
}
