mod agent;
mod app;
mod event;
mod git;
mod plugin;
mod signal;
mod state;
mod tmux;
mod ui;
mod worktree;

use anyhow::Result;
use app::{App, Mode};
use clap::{Parser, Subcommand};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use event::Event;
use ratatui::{backend::CrosstermBackend, Terminal};
use state::Config;
use std::io;

#[derive(Parser, Debug)]
#[command(name = "agent-sidebar")]
#[command(about = "A tmux sidebar for tracking coding agents", long_about = None)]
#[command(version)]
struct Args {
    #[command(subcommand)]
    command: Option<Commands>,
    
    #[arg(short, long, default_value = "30")]
    width: u16,
}

#[derive(Subcommand, Debug)]
enum Commands {
    #[command(about = "Install opencode plugin")]
    Install {
        #[arg(long, help = "Force reinstall if already installed")]
        force: bool,
    },
    #[command(about = "Uninstall opencode plugin")]
    Uninstall,
    #[command(about = "Check plugin installation status")]
    Status,
    #[command(about = "Configure agent-sidebar")]
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
}

#[derive(Subcommand, Debug)]
enum ConfigCommands {
    #[command(about = "Set worktree root directory for grouping")]
    SetWorktreeRoot {
        #[arg(help = "Path to the worktree root directory")]
        path: String,
    },
    #[command(about = "Clear worktree root configuration")]
    ClearWorktreeRoot,
    #[command(about = "Show current configuration")]
    Show,
}

fn check_tmux() -> Result<()> {
    if std::env::var("TMUX").is_err() {
        anyhow::bail!("Not running in a tmux session. agent-sidebar must be launched inside tmux.");
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    match args.command {
        Some(Commands::Install { force }) => {
            plugin::install(force)?;
            println!("Plugin installed successfully!");
            println!("Restart opencode for changes to take effect.");
        }
        Some(Commands::Uninstall) => {
            plugin::uninstall()?;
            println!("Plugin uninstalled successfully!");
        }
        Some(Commands::Status) => {
            plugin::status()?;
        }
        Some(Commands::Config { command }) => {
            handle_config_command(command)?;
        }
        None => {
            run_tui(args.width).await?;
        }
    }

    Ok(())
}

fn handle_config_command(command: ConfigCommands) -> Result<()> {
    match command {
        ConfigCommands::SetWorktreeRoot { path } => {
            let expanded = if path.starts_with('~') {
                if let Some(home) = dirs::home_dir() {
                    path.replacen('~', &home.to_string_lossy(), 1)
                } else {
                    path.clone()
                }
            } else {
                path.clone()
            };
            
            let abs_path = std::fs::canonicalize(&expanded)?;
            
            let mut config = Config::load().unwrap_or_default();
            config.worktree_root = Some(abs_path.to_string_lossy().to_string());
            config.save()?;
            
            println!("Worktree root set to: {}", abs_path.display());
        }
        ConfigCommands::ClearWorktreeRoot => {
            let mut config = Config::load().unwrap_or_default();
            config.worktree_root = None;
            config.save()?;
            
            println!("Worktree root cleared.");
        }
        ConfigCommands::Show => {
            let config = Config::load().unwrap_or_default();
            
            println!("Current configuration:");
            match config.worktree_root {
                Some(root) => println!("  worktree_root: {}", root),
                None => println!("  worktree_root: (not set)"),
            }
        }
    }
    
    Ok(())
}

async fn run_tui(width: u16) -> Result<()> {
    check_tmux()?;
    let _width = width;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let mut event_stream = event::EventStream::new();

    let res = run_app(&mut terminal, &mut app, &mut event_stream).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {err:?}");
        return Err(err);
    }

    Ok(())
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    event_stream: &mut event::EventStream,
) -> Result<()> {
    loop {
        terminal.draw(|f| ui::render(f, app))?;

        match event_stream.next().await {
            Some(Event::Key(key)) => {
                match app.mode {
                    Mode::Normal => match key.code {
                        crossterm::event::KeyCode::Char('q') | crossterm::event::KeyCode::Esc => {
                            return Ok(())
                        }
                        crossterm::event::KeyCode::Char('j') => {
                            app.select_next();
                        }
                        crossterm::event::KeyCode::Char('k') => {
                            app.select_prev();
                        }
                        crossterm::event::KeyCode::Char('?') => {
                            app.mode = Mode::Help;
                        }
                        crossterm::event::KeyCode::Char('r') => {
                            app.start_rename();
                        }
                        crossterm::event::KeyCode::Char('d') => {
                            app.start_diff_view();
                        }
                        crossterm::event::KeyCode::Enter => {
                            if let Err(e) = app.jump_to_selected() {
                                eprintln!("Failed to jump to pane: {}", e);
                            }
                        }
                        _ => {}
                    },
                    Mode::Rename { .. } => match key.code {
                        crossterm::event::KeyCode::Enter => {
                            if let Err(e) = app.confirm_rename() {
                                eprintln!("Failed to save rename: {}", e);
                            }
                        }
                        crossterm::event::KeyCode::Esc => {
                            app.cancel_rename();
                        }
                        crossterm::event::KeyCode::Backspace => {
                            app.handle_rename_backspace();
                        }
                        crossterm::event::KeyCode::Char(c) => {
                            app.handle_rename_input(c);
                        }
                        _ => {}
                    },
                    Mode::Help => {
                        app.mode = Mode::Normal;
                    }
                    Mode::DiffView { .. } => {
                        app.mode = Mode::Normal;
                    }
                }
            }
            Some(Event::Tick) => {
                app.tick();
            }
            None => {}
        }
    }
}
