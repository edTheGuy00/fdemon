//! Terminal event polling

use crate::app::message::Message;
use crate::common::prelude::*;
use crossterm::event::{self, Event};
use std::time::Duration;

/// Poll for terminal events with timeout
pub fn poll() -> Result<Option<Message>> {
    // Poll with 50ms timeout (20 FPS)
    if event::poll(Duration::from_millis(50))? {
        match event::read()? {
            Event::Key(key) if key.kind == event::KeyEventKind::Press => {
                Ok(Some(Message::Key(key)))
            }
            _ => Ok(None),
        }
    } else {
        // Generate tick on timeout for animations
        Ok(Some(Message::Tick))
    }
}
