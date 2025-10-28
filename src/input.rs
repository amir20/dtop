use crossterm::event::{self, Event, KeyCode};
use std::time::Duration;

use crate::types::{AppEvent, EventSender};

/// Polls for keyboard input and terminal events
/// Sends events for various key presses and terminal resize
pub fn keyboard_worker(tx: EventSender) {
    loop {
        // Poll every 200ms - humans won't notice the difference
        if event::poll(Duration::from_millis(200)).unwrap_or(false)
            && let Ok(event) = event::read()
        {
            match event {
                Event::Key(key) => match key.code {
                    KeyCode::Char('q') | KeyCode::Char('c')
                        if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                    {
                        let _ = tx.blocking_send(AppEvent::Quit);
                        break;
                    }
                    KeyCode::Char('q') => {
                        let _ = tx.blocking_send(AppEvent::Quit);
                        break;
                    }
                    KeyCode::Up => {
                        // Send both events - handler will decide based on view state
                        let _ = tx.blocking_send(AppEvent::SelectPrevious);
                        let _ = tx.blocking_send(AppEvent::ScrollUp);
                    }
                    KeyCode::Down => {
                        // Send both events - handler will decide based on view state
                        let _ = tx.blocking_send(AppEvent::SelectNext);
                        let _ = tx.blocking_send(AppEvent::ScrollDown);
                    }
                    KeyCode::Enter => {
                        let _ = tx.blocking_send(AppEvent::EnterPressed);
                    }
                    KeyCode::Esc => {
                        let _ = tx.blocking_send(AppEvent::ExitLogView);
                    }
                    _ => {}
                },
                Event::Resize(_, _) => {
                    let _ = tx.blocking_send(AppEvent::Resize);
                }
                _ => {}
            }
        }
    }
}
