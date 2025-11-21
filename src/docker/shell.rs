use bollard::exec::{CreateExecOptions, ResizeExecOptions, StartExecOptions, StartExecResults};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures_util::StreamExt;
use std::io::{self, Write};
use tokio::io::AsyncWriteExt as _;

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
    println!("Connecting to shell in container {}...", container_id);

    // Get terminal size
    let (cols, rows) = terminal::size()?;

    // Create exec instance with /bin/sh (most containers have this)
    let exec_config = CreateExecOptions {
        cmd: Some(vec!["/bin/sh"]),
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

            // Spawn task to read from container and write to stdout
            let output_handle = tokio::spawn(async move {
                let mut stdout = io::stdout();
                while let Some(result) = output.next().await {
                    match result {
                        Ok(output) => {
                            let bytes = output.into_bytes();
                            let _ = stdout.write_all(&bytes);
                            let _ = stdout.flush();
                        }
                        Err(_) => break,
                    }
                }
            });

            // Read from stdin and write to container
            loop {
                if event::poll(std::time::Duration::from_millis(1))? {
                    match event::read()? {
                        Event::Key(key_event) => {
                            // Check for Ctrl-D or Ctrl-C to potentially exit
                            let bytes = match key_event.code {
                                KeyCode::Char('d')
                                    if key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                                {
                                    vec![4] // EOT
                                }
                                KeyCode::Char('c')
                                    if key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                                {
                                    vec![3] // ETX
                                }
                                KeyCode::Char(c)
                                    if key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                                {
                                    // Convert Ctrl+letter to control character
                                    vec![(c as u8) & 0x1f]
                                }
                                KeyCode::Char(c) => c.to_string().into_bytes(),
                                KeyCode::Enter => vec![13],
                                KeyCode::Backspace => vec![127],
                                KeyCode::Tab => vec![9],
                                KeyCode::Esc => vec![27],
                                KeyCode::Up => vec![27, 91, 65],
                                KeyCode::Down => vec![27, 91, 66],
                                KeyCode::Right => vec![27, 91, 67],
                                KeyCode::Left => vec![27, 91, 68],
                                KeyCode::Home => vec![27, 91, 72],
                                KeyCode::End => vec![27, 91, 70],
                                KeyCode::Delete => vec![27, 91, 51, 126],
                                _ => continue,
                            };

                            if input.write_all(&bytes).await.is_err() {
                                break;
                            }
                            if input.flush().await.is_err() {
                                break;
                            }
                        }
                        Event::Resize(cols, rows) => {
                            // Resize the TTY
                            let resize_options = ResizeExecOptions {
                                height: rows,
                                width: cols,
                            };
                            let _ = host.docker.resize_exec(&exec_id, resize_options).await;
                        }
                        _ => {}
                    }
                }

                // Check if output task has finished (shell exited)
                if output_handle.is_finished() {
                    break;
                }
            }

            // Wait for output task to complete
            let _ = output_handle.await;
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
