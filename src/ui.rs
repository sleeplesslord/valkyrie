use crate::app::{App, Mode};
use chrono::Utc;
use ratatui::style::Stylize;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

pub fn render(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(f.area());

    render_header(f, app, chunks[0]);
    render_agent_list(f, app, chunks[1]);
    render_footer(f, app, chunks[2]);

    match &app.mode {
        Mode::Help => {
            render_help_overlay(f);
        }
        Mode::Rename { .. } => {
            render_rename_input(f, app);
        }
        Mode::DiffView { .. } => {
            render_diff_view(f, app);
        }
        _ => {}
    }
}

fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let title = match app.mode {
        Mode::Normal => " valkyrie ",
        Mode::Rename { .. } => " rename agent ",
        Mode::Help => " help ",
        Mode::DiffView { .. } => " diff view ",
    };

    let header = Paragraph::new(title).style(Style::default().fg(Color::Cyan).bold());
    f.render_widget(header, area);
}

fn render_agent_list(f: &mut Frame, app: &App, area: Rect) {
    if app.agents.is_empty() {
        let empty_msg =
            Paragraph::new("No agents detected").style(Style::default().fg(Color::DarkGray));
        f.render_widget(empty_msg, area);
        return;
    }

    let width = area.width as usize;
    let groups = app.agents_by_worktree();
    let mut items: Vec<ListItem> = Vec::new();
    let mut agent_index = 0;

    for (worktree, agents) in groups {
        if let Some(ref wt_name) = worktree {
            let header_style = Style::default().fg(Color::Blue).bold();
            let display_name = if wt_name.is_empty() {
                "◆ (root)".to_string()
            } else {
                format!("◆ {}", wt_name)
            };
            items.push(ListItem::new(Line::styled(display_name, header_style)));
        }

        for agent in agents {
            // Per-activity color for Running; distinct colors per status otherwise
            let status_color = match agent.status {
                crate::agent::AgentStatus::Running => {
                    match agent.activity.as_deref() {
                        Some("coding") => Color::Rgb(80, 250, 123),
                        Some("exploring") => Color::Rgb(0, 245, 255),
                        Some("running") => Color::Rgb(241, 250, 140),
                        Some("researching") => Color::Rgb(255, 121, 198),
                        Some("thinking") => Color::White,
                        _ => Color::Rgb(80, 250, 123),
                    }
                }
                crate::agent::AgentStatus::Idle => Color::Rgb(150, 150, 150),
                crate::agent::AgentStatus::WaitingInput => Color::Rgb(241, 250, 140),
                crate::agent::AgentStatus::Error => Color::Rgb(255, 85, 85),
                crate::agent::AgentStatus::Offline => Color::DarkGray,
                crate::agent::AgentStatus::Unknown => Color::Blue,
            };

            let is_selected = agent_index == app.selection;

            let name_color = match agent.agent_type {
                crate::agent::AgentType::Opencode => Color::Cyan,
                crate::agent::AgentType::ClaudeCode => Color::Magenta,
            };

            let status_indicator = agent.status.indicator(
                agent.activity.as_deref(),
                agent.tool_executing.as_deref(),
                app.tick_count,
            );

            let prefix = if worktree.is_some() { "  " } else { "" };

            let indicator_width = status_indicator.chars().count();
            let name_max = width.saturating_sub(prefix.len() + indicator_width + 2);
            let name = truncate_str(&agent.name, name_max);

            let (indicator_style, name_style) = if is_selected {
                (
                    Style::default().fg(status_color).bold(),
                    Style::default().fg(name_color).bold(),
                )
            } else {
                (
                    Style::default().fg(status_color).bold(),
                    Style::default().fg(name_color),
                )
            };

            let rel_time = format_relative_time(&agent.last_activity);
            let line1 = Line::from(vec![
                Span::styled(format!("{}{} ", prefix, status_indicator), indicator_style),
                Span::styled(name, name_style),
                Span::styled(format!(" {}", rel_time), Style::default().fg(Color::DarkGray)),
            ]);
            items.push(ListItem::new(line1));

            let has_task = agent
                .task_description
                .as_deref()
                .map(|t| !t.is_empty())
                .unwrap_or(false);
            let diff = agent.diff_stats.as_ref().map(|d| d.to_string());
            let has_diff = diff.as_deref().map(|d| !d.is_empty()).unwrap_or(false);
            let has_file = agent.current_file.as_deref().map(|f| !f.is_empty()).unwrap_or(false);

            if has_task || has_diff || has_file {
                let indent = if worktree.is_some() { "    " } else { "  " };
                let sub_style = |fg: Color| Style::default().fg(fg);

                // --- Task + diff line ---
                if has_task || has_diff {
                    let mut spans: Vec<Span> = vec![Span::styled(indent.to_string(), sub_style(Color::DarkGray))];

                    // Compute diff display parts and their width
                    let files_changed = agent.diff_stats.as_ref().map(|d| d.files_changed).unwrap_or(0);
                    let (additions, deletions) = if has_diff {
                        let diff_str = diff.as_deref().unwrap_or("");
                        parse_diff_stats(diff_str)
                    } else {
                        (String::new(), String::new())
                    };

                    // Build the diff suffix: ◇3 +42 -10
                    let mut diff_parts: Vec<(String, Color)> = Vec::new();
                    if files_changed > 0 {
                        diff_parts.push((format!("◇{}", files_changed), Color::DarkGray));
                    }
                    if !additions.is_empty() {
                        diff_parts.push((format!("+{}", additions), Color::Green));
                    }
                    if !deletions.is_empty() {
                        diff_parts.push((format!("-{}", deletions), Color::Red));
                    }
                    let diff_display_len: usize = diff_parts.iter().map(|(s, _)| s.chars().count()).sum::<usize>()
                        + diff_parts.len().saturating_sub(1); // spaces between parts

                    if has_task {
                        let task_max = width.saturating_sub(indent.len() + diff_display_len + 1);
                        let task =
                            truncate_str(agent.task_description.as_deref().unwrap_or(""), task_max);
                        spans.push(Span::styled(task, sub_style(Color::Gray)));
                        if !diff_parts.is_empty() {
                            spans.push(Span::styled(" ".to_string(), sub_style(Color::DarkGray)));
                        }
                    }
                    for (i, (text, color)) in diff_parts.iter().enumerate() {
                        if i > 0 {
                            spans.push(Span::styled(" ".to_string(), sub_style(Color::DarkGray)));
                        }
                        spans.push(Span::styled(text.clone(), sub_style(*color)));
                    }

                    items.push(ListItem::new(Line::from(spans)));
                }

                // --- Current file line ---
                if has_file {
                    let file_path = shorten_path(
                        agent.current_file.as_deref().unwrap_or(""),
                        &agent.working_dir,
                    );
                    let file_max = width.saturating_sub(indent.len() + 2); // "✎ " prefix
                    let file_display = truncate_str(&file_path, file_max);
                    let file_line = Line::from(vec![
                        Span::styled(indent.to_string(), sub_style(Color::DarkGray)),
                        Span::styled("✎ ".to_string(), sub_style(Color::DarkGray)),
                        Span::styled(file_display, sub_style(Color::Gray)),
                    ]);
                    items.push(ListItem::new(file_line));
                }
            }

            agent_index += 1;

            for saga in agent.sagas.iter().take(3) {
                let saga_indent = if worktree.is_some() { "      " } else { "    " };
                let (saga_status_str, saga_status_color) = match saga.status.as_str() {
                    "active" => ("●", Color::Green),
                    "claimed" => ("◐", Color::Yellow),
                    "done" => ("✓", Color::DarkGray),
                    _ => ("?", Color::DarkGray),
                };
                let saga_title_color = if saga.status.as_str() == "done" {
                    Color::DarkGray
                } else {
                    Color::Gray
                };
                let used = saga_indent.len() + saga_status_str.len() + 1;
                let saga_title_max = width.saturating_sub(used);
                let saga_title = truncate_str(&saga.title, saga_title_max);
                let saga_style = |fg: Color| Style::default().fg(fg);
                let saga_line = Line::from(vec![
                    Span::styled(saga_indent.to_string(), saga_style(Color::DarkGray)),
                    Span::styled(
                        saga_status_str.to_string(),
                        saga_style(saga_status_color),
                    ),
                    Span::styled(" ".to_string(), saga_style(Color::DarkGray)),
                    Span::styled(saga_title, saga_style(saga_title_color)),
                ]);
                items.push(ListItem::new(saga_line));
            }
        }

        // Add a thin separator between worktree groups
        if !items.is_empty() {
            let separator = "─".repeat(width);
            items.push(ListItem::new(Line::styled(
                separator,
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    // Remove the trailing separator after the last group
    if !items.is_empty() {
        items.pop();
    }

    let list = List::new(items).block(Block::default().borders(Borders::NONE));

    f.render_widget(list, area);
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if max_len == 0 {
        return String::new();
    }
    let display_len = s.chars().count();
    if display_len > max_len {
        let truncated: String = s.chars().take(max_len.saturating_sub(1)).collect();
        format!("{}…", truncated)
    } else {
        s.to_string()
    }
}

fn format_relative_time(dt: &chrono::DateTime<chrono::Utc>) -> String {
    let now = Utc::now();
    let delta = now.signed_duration_since(*dt);
    let secs = delta.num_seconds();
    if secs < 0 {
        return "now".to_string();
    }
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86400 {
        format!("{}h", secs / 3600)
    } else {
        format!("{}d", secs / 86400)
    }
}

/// Strip the working_dir prefix from a file path to show just the
/// project-relative portion. Falls back to the filename if the path
/// is too long even after stripping.
fn shorten_path(file: &str, working_dir: &str) -> String {
    let stripped = if !working_dir.is_empty() {
        file.strip_prefix(working_dir)
            .or_else(|| file.strip_prefix(&format!("{}/", working_dir)))
            .unwrap_or(file)
    } else {
        file
    };
    let stripped = stripped.strip_prefix('/').unwrap_or(stripped);
    // If still too long, show just the filename
    if stripped.chars().count() > 40 {
        std::path::Path::new(stripped)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| truncated_path(stripped))
    } else {
        stripped.to_string()
    }
}

fn truncated_path(s: &str) -> String {
    let len = s.chars().count();
    if len > 20 {
        let tail: String = s.chars().skip(len - 20).collect();
        format!("…{}", tail)
    } else {
        s.to_string()
    }
}

fn parse_diff_stats(diff: &str) -> (String, String) {
    let mut additions = String::new();
    let mut deletions = String::new();
    for part in diff.split_whitespace() {
        if let Some(num) = part.strip_prefix('+') {
            additions = num.to_string();
        } else if let Some(num) = part.strip_prefix('-') {
            deletions = num.to_string();
        }
    }
    (additions, deletions)
}

fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    let help_text = match app.mode {
        Mode::Normal => {
            " j/k:nav | Enter:jump | w:worktree | r:rename | d:diff | D:diff window | ?:help | q:quit "
        }
        Mode::Rename { .. } => " Enter:save | Esc:cancel ",
        Mode::Help => " any key to close ",
        Mode::DiffView { .. } => " Esc:back ",
    };

    let footer = Paragraph::new(help_text).style(Style::default().fg(Color::DarkGray));
    f.render_widget(footer, area);
}

fn render_help_overlay(f: &mut Frame) {
    let area = centered_rect(60, 50, f.area());

    let help_text = vec![
        Line::from(Span::styled("Keybindings", Style::default().bold())),
        Line::from(""),
        Line::from(" j/k     Navigate agents"),
        Line::from(" Enter   Jump to agent pane"),
        Line::from(" w       Open worktree in new window"),
        Line::from(" r       Rename agent"),
        Line::from(" d       View git diff (overlay)"),
        Line::from(" D       Open diff in new window"),
        Line::from(" ?       Toggle this help"),
        Line::from(" q/Esc   Quit"),
    ];

    let help = Paragraph::new(help_text).block(
        Block::default()
            .title(" Help ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    f.render_widget(help, area);
}

fn render_rename_input(f: &mut Frame, app: &App) {
    let area = centered_rect(70, 20, f.area());

    let input = Paragraph::new(app.input_buffer.as_str())
        .style(Style::default().fg(Color::Yellow))
        .block(
            Block::default()
                .title(" Rename Agent ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        );

    f.render_widget(input, area);
}

fn render_diff_view(f: &mut Frame, app: &App) {
    let area = centered_rect(90, 80, f.area());

    let max_lines = area.height.saturating_sub(2) as usize;
    let all_lines: Vec<&str> = app.input_buffer.lines().collect();
    let total_lines = all_lines.len();
    let scroll = app.diff_scroll.min(all_lines.len().saturating_sub(1));

    let diff_text: Vec<Line> = all_lines
        .into_iter()
        .skip(scroll)
        .take(max_lines)
        .map(|line| {
            let style = if line.starts_with("diff --git") {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else if line.starts_with("---") {
                Style::default().fg(Color::Magenta)
            } else if line.starts_with("+++") {
                Style::default().fg(Color::Magenta)
            } else if line.starts_with("@@") {
                Style::default().fg(Color::Cyan)
            } else if line.starts_with('+') {
                Style::default().fg(Color::Green)
            } else if line.starts_with('-') {
                Style::default().fg(Color::Red)
            } else {
                Style::default().fg(Color::White)
            };
            Line::styled(line, style)
        })
        .collect();

    let title = format!(
        " Git Diff (Esc to close) {}/{} ",
        scroll + max_lines.min(total_lines - scroll),
        total_lines
    );
    let diff = Paragraph::new(diff_text).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    f.render_widget(diff, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
