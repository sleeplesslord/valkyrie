use crate::app::{App, Mode};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use ratatui::style::Stylize;

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
        Mode::Normal => " agent-sidebar ",
        Mode::Rename { .. } => " rename agent ",
        Mode::Help => " help ",
        Mode::DiffView { .. } => " diff view ",
    };

    let header = Paragraph::new(title)
        .style(Style::default().fg(Color::Cyan).bold());
    f.render_widget(header, area);
}

fn render_agent_list(f: &mut Frame, app: &App, area: Rect) {
    if app.agents.is_empty() {
        let empty_msg = Paragraph::new("No agents detected")
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(empty_msg, area);
        return;
    }

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
            let base_style = match agent.status {
                crate::agent::AgentStatus::Running => Style::default().fg(Color::Green),
                crate::agent::AgentStatus::Idle => Style::default().fg(Color::Gray),
                crate::agent::AgentStatus::WaitingInput => Style::default().fg(Color::Yellow),
                crate::agent::AgentStatus::Error => Style::default().fg(Color::Red),
                crate::agent::AgentStatus::Offline => Style::default().fg(Color::DarkGray),
                crate::agent::AgentStatus::Unknown => Style::default().fg(Color::Blue),
            };

            let is_selected = agent_index == app.selection;
            let style = if is_selected {
                base_style.bold()
            } else {
                base_style
            };

            let status_indicator = agent.status.indicator();
            let type_indicator = match agent.agent_type {
                crate::agent::AgentType::Opencode => "O",
                crate::agent::AgentType::ClaudeCode => "C",
                crate::agent::AgentType::Aider => "A",
                crate::agent::AgentType::Generic => "G",
            };
            
            let name = truncate_str(&agent.name, 12);
            let task = agent.task_description.as_deref().map(|t| truncate_str(t, 15));
            let diff = agent.diff_stats.as_ref().map(|d| d.to_string());

            let prefix = if worktree.is_some() { "  " } else { "" };

            let content = match (task, diff) {
                (Some(t), Some(d)) if !d.is_empty() => {
                    format!("{}{} [{}] {} - {} [{}]", prefix, status_indicator, type_indicator, name, t, d)
                }
                (Some(t), _) => format!("{}{} [{}] {} - {}", prefix, status_indicator, type_indicator, name, t),
                (None, Some(d)) if !d.is_empty() => {
                    format!("{}{} [{}] {} [{}]", prefix, status_indicator, type_indicator, name, d)
                }
                _ => format!("{}{} [{}] {}", prefix, status_indicator, type_indicator, name),
            };

            items.push(ListItem::new(Line::from(Span::styled(content, style))));
            agent_index += 1;
        }

        items.push(ListItem::new(Line::from("")));
    }

    if !items.is_empty() {
        items.pop();
    }

    let list = List::new(items)
        .block(Block::default().borders(Borders::NONE));

    f.render_widget(list, area);
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}…", &s[..max_len.saturating_sub(1)])
    } else {
        s.to_string()
    }
}

fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    let help_text = match app.mode {
        Mode::Normal => " j/k:nav | Enter:jump | r:rename | d:diff | D:diff window | ?:help | q:quit ",
        Mode::Rename { .. } => " Enter:save | Esc:cancel ",
        Mode::Help => " any key to close ",
        Mode::DiffView { .. } => " Esc:back ",
    };

    let footer = Paragraph::new(help_text)
        .style(Style::default().fg(Color::DarkGray));
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

    let help = Paragraph::new(help_text)
        .block(
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
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
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

    let title = format!(" Git Diff (Esc to close) {}/{} ", scroll + max_lines.min(total_lines - scroll), total_lines);
    let diff = Paragraph::new(diff_text)
        .block(
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
