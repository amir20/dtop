use bollard::exec::{CreateExecOptions, ResizeExecOptions, StartExecOptions, StartExecResults};
use crossterm::{
    cursor,
    event::{Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures_util::StreamExt;
use std::io;
use tokio::io::AsyncWriteExt as _;
use tokio::sync::mpsc;

use crate::docker::connection::DockerHost;

/// Runs an interactive shell session inside a container
/// This function takes over the terminal completely until the shell exits
pub async fn run_shell_session(
    host: &DockerHost,
    container_id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use tracing::debug;

    debug!("Starting shell session for container: {}", container_id);

    // Leave alternate screen so shell output is visible and show cursor
    let mut stdout = io::stdout();
    execute!(stdout, LeaveAlternateScreen, cursor::Show)?;
    terminal::disable_raw_mode()?;

    // Print a message so user knows shell is starting
    println!();
    println!("Connecting to shell in container {}...", container_id);
    println!("Press Ctrl+D to exit");
    println!();

    // Get terminal size
    let (cols, rows) = terminal::size()?;

    // Create exec instance with /bin/sh (most containers have this)
    let exec_config = CreateExecOptions {
        cmd: Some(vec![
            "sh",
            "-c",
            "command -v bash >/dev/null 2>&1 && exec bash || exec sh",
        ]),
        attach_stdin: Some(true),
        attach_stdout: Some(true),
        attach_stderr: Some(true),
        tty: Some(true),
        env: Some(vec!["TERM=xterm-256color"]),
        ..Default::default()
    };

    let exec_instance = host
        .docker
        .create_exec(container_id, exec_config)
        .await
        .map_err(|e| format!("Failed to create exec: {}", e))?;

    let exec_id = exec_instance.id;
    debug!("Created exec instance: {}", exec_id);

    // Start the exec session
    let start_config = StartExecOptions {
        detach: false,
        tty: true,
        ..Default::default()
    };

    debug!("Starting exec session...");
    let exec_result = host
        .docker
        .start_exec(&exec_id, Some(start_config))
        .await
        .map_err(|e| format!("Failed to start exec: {}", e))?;

    debug!("Exec started, handling attached session");

    // Resize the TTY to match terminal size (after exec starts)
    let resize_options = ResizeExecOptions {
        height: rows,
        width: cols,
    };
    let _ = host.docker.resize_exec(&exec_id, resize_options).await;

    // Handle the attached session
    match exec_result {
        StartExecResults::Attached {
            mut output,
            mut input,
        } => {
            debug!("Got attached session with input/output streams");
            // Enable raw mode for the shell session
            terminal::enable_raw_mode()?;

            // Create channel for input events from blocking thread
            let (input_tx, mut input_rx) = mpsc::channel::<InputEvent>(32);

            // Spawn blocking thread for crossterm event reading
            let input_handle = std::thread::spawn(move || {
                loop {
                    // 100ms poll timeout - human input doesn't need 1ms responsiveness
                    if crossterm::event::poll(std::time::Duration::from_millis(100))
                        .unwrap_or(false)
                    {
                        match crossterm::event::read() {
                            Ok(event) => {
                                if input_tx.blocking_send(InputEvent::Event(event)).is_err() {
                                    break; // Channel closed, exit thread
                                }
                            }
                            Err(_) => break,
                        }
                    }

                    // Check if we should shutdown (channel closed)
                    if input_tx.is_closed() {
                        break;
                    }
                }
            });

            // Spawn async task to read from container and write to stdout
            let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
            let output_handle = tokio::spawn(async move {
                let mut stdout = tokio::io::stdout();
                loop {
                    tokio::select! {
                        biased;
                        _ = shutdown_rx.recv() => break,
                        result = output.next() => {
                            match result {
                                Some(Ok(output)) => {
                                    let bytes = output.into_bytes();
                                    if stdout.write_all(&bytes).await.is_err() {
                                        break;
                                    }
                                    if stdout.flush().await.is_err() {
                                        break;
                                    }
                                }
                                Some(Err(_)) | None => break,
                            }
                        }
                    }
                }
            });

            // Main async loop to process input events and send to container
            let exec_id_clone = exec_id.clone();
            let docker_clone = host.docker.clone();
            loop {
                tokio::select! {
                    biased;
                    // Check if output task finished (shell exited)
                    _ = async {
                        while !output_handle.is_finished() {
                            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                        }
                    } => {
                        break;
                    }
                    // Process input events from the blocking thread
                    event = input_rx.recv() => {
                        match event {
                            Some(InputEvent::Event(Event::Key(key_event))) => {
                                let Some(bytes) = key_to_bytes(key_event) else {
                                    continue;
                                };

                                if input.write_all(&bytes).await.is_err() {
                                    break;
                                }
                                if input.flush().await.is_err() {
                                    break;
                                }
                            }
                            Some(InputEvent::Event(Event::Resize(cols, rows))) => {
                                let resize_options = ResizeExecOptions {
                                    height: rows,
                                    width: cols,
                                };
                                let _ = docker_clone.resize_exec(&exec_id_clone, resize_options).await;
                            }
                            Some(InputEvent::Event(_)) => {}
                            None => break, // Input channel closed
                        }
                    }
                }
            }

            // Signal output task to shutdown and wait for completion
            let _ = shutdown_tx.send(()).await;
            let _ = output_handle.await;

            // Input thread will exit when channel is dropped
            drop(input_rx);
            let _ = input_handle.join();
        }
        StartExecResults::Detached => {
            return Err("Exec started in detached mode unexpectedly".into());
        }
    }

    // Restore terminal state
    terminal::disable_raw_mode()?;
    execute!(
        io::stdout(),
        EnterAlternateScreen,
        Clear(ClearType::All),
        cursor::Hide
    )?;
    terminal::enable_raw_mode()?;

    Ok(())
}

/// Input events from the blocking crossterm thread
enum InputEvent {
    Event(Event),
}

/// Convert a key event to bytes to send to the container
fn key_to_bytes(key_event: KeyEvent) -> Option<Vec<u8>> {
    use KeyCode::*;

    Some(match key_event.code {
        Char(c) if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
            if c == 'd' {
                return Some(vec![4]);
            } // Special case common ones
            if c == 'c' {
                return Some(vec![3]);
            }
            vec![(c as u8) & 0x1f]
        }
        Char(c) => c.to_string().into_bytes(),
        Enter => vec![b'\r'],
        Backspace => vec![0x7f],
        Tab => vec![b'\t'],
        Esc => vec![0x1b],
        Up => b"\x1b[A".to_vec(),
        Down => b"\x1b[B".to_vec(),
        Right => b"\x1b[C".to_vec(),
        Left => b"\x1b[D".to_vec(),
        Home => b"\x1b[H".to_vec(),
        End => b"\x1b[F".to_vec(),
        Delete => b"\x1b[3~".to_vec(),
        _ => return None,
    })
}
