use crossterm::event::{self, Event, KeyCode, MouseEventKind};
use std::time::Duration;

use crate::types::{AppEvent, EventSender};

/// Polls for keyboard input and terminal events
/// Sends events for various key presses, mouse events, and terminal resize
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
                    KeyCode::Up | KeyCode::Char('k') => {
                        // Send both events - handler will decide based on view state
                        let _ = tx.blocking_send(AppEvent::SelectPrevious);
                        let _ = tx.blocking_send(AppEvent::ScrollUp);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
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
                        let _ = tx
                            .blocking_send(AppEvent::SetSortField(crate::types::SortField::Uptime));
                    }
                    KeyCode::Char('n') | KeyCode::Char('N') => {
                        let _ =
                            tx.blocking_send(AppEvent::SetSortField(crate::types::SortField::Name));
                    }
                    KeyCode::Char('c') | KeyCode::Char('C') => {
                        let _ =
                            tx.blocking_send(AppEvent::SetSortField(crate::types::SortField::Cpu));
                    }
                    KeyCode::Char('m') | KeyCode::Char('M') => {
                        let _ = tx
                            .blocking_send(AppEvent::SetSortField(crate::types::SortField::Memory));
                    }
                    KeyCode::Char('a') | KeyCode::Char('A') => {
                        let _ = tx.blocking_send(AppEvent::ToggleShowAll);
                    }
                    _ => {}
                },
                Event::Resize(_, _) => {
                    let _ = tx.blocking_send(AppEvent::Resize);
                }
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollUp => {
                        // Send both events - handler will decide based on view state
                        let _ = tx.blocking_send(AppEvent::SelectPrevious);
                        let _ = tx.blocking_send(AppEvent::ScrollUp);
                    }
                    MouseEventKind::ScrollDown => {
                        // Send both events - handler will decide based on view state
                        let _ = tx.blocking_send(AppEvent::SelectNext);
                        let _ = tx.blocking_send(AppEvent::ScrollDown);
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }
}
