use crossterm::event::{self, Event, KeyCode, KeyEvent};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::core::types::{AppEvent, EventSender, SortField};

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
                    handle_key_event(key, &tx);
                }
                Event::Resize(_, _) => {
                    let _ = tx.blocking_send(AppEvent::Resize);
                }
                _ => {}
            }
        }
    }
}

fn handle_key_event(key: KeyEvent, tx: &EventSender) {
    // Ctrl+C / Ctrl+Q - always quit immediately (bypasses search mode)
    if matches!(key.code, KeyCode::Char('q') | KeyCode::Char('c'))
        && key.modifiers.contains(event::KeyModifiers::CONTROL)
    {
        let _ = tx.blocking_send(AppEvent::Quit);
        return;
    }

    // Send SearchKeyEvent first - AppState will handle it if search is active
    let _ = tx.blocking_send(AppEvent::SearchKeyEvent(key));

    // Then send specific events for known shortcuts
    // (AppState will ignore these if search mode consumed the key)
    match key.code {
        KeyCode::Char('q') => {
            let _ = tx.blocking_send(AppEvent::Quit);
        }
        // Ctrl+U for page up in log view
        KeyCode::Char('u') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
            let _ = tx.blocking_send(AppEvent::ScrollPageUp);
        }
        // Ctrl+D for page down in log view
        KeyCode::Char('d') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
            let _ = tx.blocking_send(AppEvent::ScrollPageDown);
        }
        KeyCode::Char('/') => {
            let _ = tx.blocking_send(AppEvent::EnterSearchMode);
        }
        KeyCode::Up | KeyCode::Char('k') => {
            // Send multiple events - handler will decide based on view state
            let _ = tx.blocking_send(AppEvent::SelectPrevious);
            let _ = tx.blocking_send(AppEvent::ScrollUp);
            let _ = tx.blocking_send(AppEvent::SelectActionUp);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            // Send multiple events - handler will decide based on view state
            let _ = tx.blocking_send(AppEvent::SelectNext);
            let _ = tx.blocking_send(AppEvent::ScrollDown);
            let _ = tx.blocking_send(AppEvent::SelectActionDown);
        }
        KeyCode::Enter => {
            // Send EnterPressed - handler will show action menu or execute action based on view state
            let _ = tx.blocking_send(AppEvent::EnterPressed);
        }
        KeyCode::Esc => {
            // Send both events - handler will decide based on view state
            let _ = tx.blocking_send(AppEvent::CancelActionMenu);
        }
        KeyCode::Char('o') => {
            let _ = tx.blocking_send(AppEvent::OpenDozzle);
        }
        KeyCode::Char('?') => {
            let _ = tx.blocking_send(AppEvent::ToggleHelp);
        }
        KeyCode::Char('s') => {
            let _ = tx.blocking_send(AppEvent::CycleSortField);
        }
        KeyCode::Char('u') | KeyCode::Char('U') => {
            let _ = tx.blocking_send(AppEvent::SetSortField(SortField::Uptime));
        }
        KeyCode::Char('n') | KeyCode::Char('N') => {
            let _ = tx.blocking_send(AppEvent::SetSortField(SortField::Name));
        }
        KeyCode::Char('c') | KeyCode::Char('C') => {
            let _ = tx.blocking_send(AppEvent::SetSortField(SortField::Cpu));
        }
        KeyCode::Char('m') | KeyCode::Char('M') => {
            let _ = tx.blocking_send(AppEvent::SetSortField(SortField::Memory));
        }
        KeyCode::Char('a') | KeyCode::Char('A') => {
            let _ = tx.blocking_send(AppEvent::ToggleShowAll);
        }
        KeyCode::Right | KeyCode::Char('l') => {
            let _ = tx.blocking_send(AppEvent::ShowLogView);
        }
        KeyCode::Left | KeyCode::Char('h') => {
            let _ = tx.blocking_send(AppEvent::ExitLogView);
        }
        // g for scroll to top (vim/less style)
        KeyCode::Char('g') => {
            let _ = tx.blocking_send(AppEvent::ScrollToTop);
        }
        // G for scroll to bottom (vim/less style)
        KeyCode::Char('G') => {
            let _ = tx.blocking_send(AppEvent::ScrollToBottom);
        }
        // Space for page down (less style)
        KeyCode::Char(' ') => {
            let _ = tx.blocking_send(AppEvent::ScrollPageDown);
        }
        // b for page up (less style)
        KeyCode::Char('b') => {
            let _ = tx.blocking_send(AppEvent::ScrollPageUp);
        }
        _ => {}
    }
}
