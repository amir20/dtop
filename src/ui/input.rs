use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::core::types::{AppEvent, EventSender};

/// Polls for keyboard input and terminal events
/// Sends events for various key presses, mouse events, and terminal resize
pub fn keyboard_worker(tx: EventSender, paused: Arc<AtomicBool>) {
    loop {
        // Check if we should pause (e.g., during shell session)
        if paused.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(50));
            continue;
        }

        // Poll every 200ms - humans won't notice the difference
        if event::poll(Duration::from_millis(200)).unwrap_or(false)
            && let Ok(event) = event::read()
        {
            match event {
                Event::Key(key) => {
                    // Ctrl+C / Ctrl+Q - always quit immediately
                    if matches!(key.code, KeyCode::Char('q') | KeyCode::Char('c'))
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        let _ = tx.blocking_send(AppEvent::Quit);
                    } else {
                        // Send a single event - AppState dispatches based on view state
                        let _ = tx.blocking_send(AppEvent::KeyInput(key));
                    }
                }
                Event::Resize(_, _) => {
                    let _ = tx.blocking_send(AppEvent::Resize);
                }
                _ => {}
            }
        }
    }
}
