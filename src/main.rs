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
use std::fs::OpenOptions;
use std::io;
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;

const SIDEBAR_STATE_FILE: &str = "sidebar-pane";

const TOGGLE_SCRIPT: &str = include_str!("../scripts/toggle-sidebar.sh");
const MOVE_SCRIPT: &str = include_str!("../scripts/move-sidebar-to-current.sh");
const WINDOW_CLOSE_SCRIPT: &str = include_str!("../scripts/check-sidebar-window-close.sh");

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
    #[command(about = "Print tmux configuration for sidebar integration")]
    SetupTmux,
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
        Some(Commands::SetupTmux) => {
            print_tmux_config();
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

fn get_sidebar_state_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("~"))
        .join(".agent-sidebar")
        .join(SIDEBAR_STATE_FILE)
}

fn write_sidebar_state(pane_id: &str) -> Result<()> {
    let path = get_sidebar_state_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, pane_id)?;
    Ok(())
}

fn clear_sidebar_state() -> Result<()> {
    let path = get_sidebar_state_path();
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}

fn install_tmux_scripts() -> Result<PathBuf> {
    let script_dir = dirs::home_dir()
        .map(|h| h.join(".local").join("bin"))
        .unwrap_or_else(|| PathBuf::from("/usr/local/bin"));
    
    std::fs::create_dir_all(&script_dir)?;
    
    let toggle_script = script_dir.join("toggle-sidebar.sh");
    let move_script = script_dir.join("move-sidebar-to-current.sh");
    let window_close_script = script_dir.join("check-sidebar-window-close.sh");
    
    std::fs::write(&toggle_script, TOGGLE_SCRIPT)?;
    std::fs::write(&move_script, MOVE_SCRIPT)?;
    std::fs::write(&window_close_script, WINDOW_CLOSE_SCRIPT)?;
    
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&toggle_script, std::fs::Permissions::from_mode(0o755))?;
        std::fs::set_permissions(&move_script, std::fs::Permissions::from_mode(0o755))?;
        std::fs::set_permissions(&window_close_script, std::fs::Permissions::from_mode(0o755))?;
    }
    
    println!("Installed scripts to:");
    println!("  {}", toggle_script.display());
    println!("  {}", move_script.display());
    println!("  {}", window_close_script.display());
    println!();
    
    Ok(script_dir)
}

fn print_tmux_config() {
    match install_tmux_scripts() {
        Ok(script_dir) => {
            let toggle_script = script_dir.join("toggle-sidebar.sh");
            let move_script = script_dir.join("move-sidebar-to-current.sh");
            let window_close_script = script_dir.join("check-sidebar-window-close.sh");
            
            println!("# Add the following to your ~/.tmux.conf:");
            println!();
            println!("# Sidebar toggle keybinding");
            println!("bind s run-shell \"{}\"", toggle_script.display());
            println!();
            println!("# Move sidebar to current window when switching windows");
            println!("set-hook -g session-window-changed 'run-shell \"{}\"'", move_script.display());
            println!();
            println!("# Close window when sidebar is the only pane left");
            println!("set-hook -g pane-exited 'run-shell \"{}\"'", window_close_script.display());
            println!();
            println!("# After adding, reload tmux config with:");
            println!("tmux source ~/.tmux.conf");
        }
        Err(e) => {
            eprintln!("Failed to install scripts: {}", e);
            std::process::exit(1);
        }
    }
}

fn init_logging() -> Result<()> {
    let log_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("~"))
        .join(".agent-sidebar");
    std::fs::create_dir_all(&log_dir)?;

    let log_path = log_dir.join("sidebar.log");
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;

    unsafe {
        libc::dup2(file.as_raw_fd(), libc::STDERR_FILENO);
    }

    Ok(())
}

async fn run_tui(width: u16) -> Result<()> {
    check_tmux()?;
    let _width = width;

    init_logging()?;

    if let Some(pane_id) = tmux::Tmux::current_pane_id() {
        write_sidebar_state(&pane_id)?;
    }

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

    let _ = clear_sidebar_state();

    if let Some(pane_id) = tmux::Tmux::current_pane_id() {
        let _ = tmux::Tmux::kill_pane(&pane_id);
    }

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
                        crossterm::event::KeyCode::Char('D') => {
                            app.open_diff_in_window();
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
                    Mode::DiffView { .. } => match key.code {
                        crossterm::event::KeyCode::Esc => {
                            app.mode = Mode::Normal;
                        }
                        crossterm::event::KeyCode::Char('j') | crossterm::event::KeyCode::Down => {
                            app.diff_scroll_down();
                        }
                        crossterm::event::KeyCode::Char('k') | crossterm::event::KeyCode::Up => {
                            app.diff_scroll_up();
                        }
                        _ => {}
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
