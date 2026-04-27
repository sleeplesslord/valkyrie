use crate::app::{App, Mode};
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
                "📁 (root)".to_string()
            } else {
                format!("📁 {}", wt_name)
            };
            items.push(ListItem::new(Line::styled(display_name, header_style)));
        }

        for agent in agents {
            let status_color = match agent.status {
                crate::agent::AgentStatus::Running => Color::Green,
                crate::agent::AgentStatus::Idle => Color::Gray,
                crate::agent::AgentStatus::WaitingInput => Color::Yellow,
                crate::agent::AgentStatus::Error => Color::Red,
                crate::agent::AgentStatus::Offline => Color::DarkGray,
                crate::agent::AgentStatus::Unknown => Color::Blue,
            };

            let is_selected = agent_index == app.selection;

            let name_color = match agent.agent_type {
                crate::agent::AgentType::Opencode => Color::Cyan,
                crate::agent::AgentType::ClaudeCode => Color::Magenta,
            };

            let status_indicator = agent
                .status
                .indicator(agent.activity.as_deref(), agent.tool_executing.as_deref());

            let prefix = if worktree.is_some() { "  " } else { "" };

            let indicator_width = status_indicator.chars().count();
            let name_max = width.saturating_sub(prefix.len() + indicator_width + 2);
            let name = truncate_str(&agent.name, name_max);

            let name_style = if is_selected {
                Style::default().fg(name_color).bold()
            } else {
                Style::default().fg(name_color)
            };

            let line1 = Line::from(vec![
                Span::styled(
                    format!("{}{} ", prefix, status_indicator),
                    Style::default().fg(status_color),
                ),
                Span::styled(name, name_style),
            ]);
            items.push(ListItem::new(line1));

            let has_task = agent
                .task_description
                .as_deref()
                .map(|t| !t.is_empty())
                .unwrap_or(false);
            let diff = agent.diff_stats.as_ref().map(|d| d.to_string());
            let has_diff = diff.as_deref().map(|d| !d.is_empty()).unwrap_or(false);

            if has_task || has_diff {
                let indent = if worktree.is_some() { "    " } else { "  " };
                let mut spans: Vec<Span> = vec![Span::styled(indent.to_string(), Style::default())];

                if has_task && has_diff {
                    let diff_str = diff.as_deref().unwrap_or("");
                    let (additions, deletions) = parse_diff_stats(diff_str);
                    let diff_display_len = if !additions.is_empty() && !deletions.is_empty() {
                        additions.len() + 1 + deletions.len() + 3
                    } else if !additions.is_empty() {
                        additions.len() + 1
                    } else if !deletions.is_empty() {
                        deletions.len() + 1
                    } else {
                        0
                    };
                    let task_max = width.saturating_sub(indent.len() + diff_display_len + 1);
                    let task =
                        truncate_str(agent.task_description.as_deref().unwrap_or(""), task_max);
                    spans.push(Span::styled(task, Style::default().fg(Color::Gray)));
                    if diff_display_len > 0 {
                        spans.push(Span::styled(" ".to_string(), Style::default()));
                    }
                    if !additions.is_empty() {
                        spans.push(Span::styled(
                            format!("+{}", additions),
                            Style::default().fg(Color::Green),
                        ));
                    }
                    if !additions.is_empty() && !deletions.is_empty() {
                        spans.push(Span::styled(" ".to_string(), Style::default()));
                    }
                    if !deletions.is_empty() {
                        spans.push(Span::styled(
                            format!("-{}", deletions),
                            Style::default().fg(Color::Red),
                        ));
                    }
                } else if has_task {
                    let task_max = width.saturating_sub(indent.len());
                    let task =
                        truncate_str(agent.task_description.as_deref().unwrap_or(""), task_max);
                    spans.push(Span::styled(task, Style::default().fg(Color::Gray)));
                } else if has_diff {
                    let diff_str = diff.as_deref().unwrap_or("");
                    let (additions, deletions) = parse_diff_stats(diff_str);
                    if !additions.is_empty() {
                        spans.push(Span::styled(
                            format!("+{}", additions),
                            Style::default().fg(Color::Green),
                        ));
                    }
                    if !additions.is_empty() && !deletions.is_empty() {
                        spans.push(Span::styled(" ".to_string(), Style::default()));
                    }
                    if !deletions.is_empty() {
                        spans.push(Span::styled(
                            format!("-{}", deletions),
                            Style::default().fg(Color::Red),
                        ));
                    }
                }

                items.push(ListItem::new(Line::from(spans)));
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
                let saga_line = Line::from(vec![
                    Span::styled(saga_indent.to_string(), Style::default()),
                    Span::styled(
                        saga_status_str.to_string(),
                        Style::default().fg(saga_status_color),
                    ),
                    Span::styled(" ".to_string(), Style::default()),
                    Span::styled(saga_title, Style::default().fg(saga_title_color)),
                ]);
                items.push(ListItem::new(saga_line));
            }
        }

        items.push(ListItem::new(Line::from("")));
    }

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
            " j/k:nav | Enter:jump | r:rename | d:diff | D:diff window | ?:help | q:quit "
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
            } else if line.starts_with("---") || line.starts_with("+++") {
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
