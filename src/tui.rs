//! Terminal inspector: focusable path input, scrollable associations, and live preview.
use anyhow::Result;
use crossterm::event::{
    self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use glyfuse::{Icon, Kind, Resolver};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::{Frame, Terminal};
use std::io::{self, Stdout};

struct State {
    query: String,
    cursor: usize,
    focus: Focus,
    expanded: bool,
    selected: usize,
    scroll: usize,
    ascii: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Focus {
    Input,
    List,
}

struct Regions {
    input: Rect,
    list: Rect,
}

fn is_directory(path: &str) -> bool {
    path.ends_with('/') || path.ends_with('\\')
}

fn byte_offset(value: &str, char_index: usize) -> usize {
    value
        .char_indices()
        .nth(char_index)
        .map_or(value.len(), |(at, _)| at)
}

fn input_scroll(state: &State, columns: usize) -> usize {
    state.cursor.saturating_sub(columns.saturating_sub(1))
}

fn insert_char(state: &mut State, character: char) {
    state
        .query
        .insert(byte_offset(&state.query, state.cursor), character);
    state.cursor += 1;
}

fn delete_char(state: &mut State, backwards: bool) {
    if backwards && state.cursor > 0 {
        let start = byte_offset(&state.query, state.cursor - 1);
        let end = byte_offset(&state.query, state.cursor);
        state.query.replace_range(start..end, "");
        state.cursor -= 1;
    } else if !backwards && state.cursor < state.query.chars().count() {
        let start = byte_offset(&state.query, state.cursor);
        let end = byte_offset(&state.query, state.cursor + 1);
        state.query.replace_range(start..end, "");
    }
}

fn foreground(icon: &Icon) -> Color {
    let (r, g, b) = icon.rgb();
    Color::Rgb(r, g, b)
}

fn render(frame: &mut Frame, resolver: &Resolver, state: &State) -> Regions {
    let outer = frame.area();
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(4),
        Constraint::Length(2),
    ])
    .split(outer);
    let directory = is_directory(&state.query);
    let kind = if directory { "folder" } else { "file" };
    let input_width = usize::from(chunks[0].width.saturating_sub(2));
    let scroll = input_scroll(state, input_width);
    let shown: String = state.query.chars().skip(scroll).take(input_width).collect();
    let input_style = if state.focus == Focus::Input {
        Color::Cyan
    } else {
        Color::DarkGray
    };
    frame.render_widget(
        Paragraph::new(shown).block(
            Block::bordered()
                .title(" Path (trailing / or \\ = folder) ")
                .border_style(Style::default().fg(input_style)),
        ),
        chunks[0],
    );
    let icon = resolver.resolve(&state.query, directory, state.expanded);
    let visual = if state.ascii {
        if directory { "[D]" } else { "[F]" }
    } else {
        &icon.glyph
    };
    let line = Line::from(vec![
        Span::styled(
            format!(" {visual} "),
            Style::default()
                .fg(foreground(icon))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(
            "{}   ·   {}   ·   {}",
            state.query, icon.color, kind
        )),
    ]);
    frame.render_widget(
        Paragraph::new(line)
            .block(Block::bordered().title(" Live match · icon colored independently of name ")),
        chunks[1],
    );

    let region = chunks[2];
    let title = format!(
        " {} · {} associations · Up/Down or wheel to browse · click to preview ",
        resolver.theme.name,
        resolver.theme.rules.len()
    );
    let list_style = if state.focus == Focus::List {
        Color::Cyan
    } else {
        Color::DarkGray
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(list_style));
    let inner = block.inner(region);
    frame.render_widget(block, region);
    let visible = usize::from(inner.height);
    let list_width = inner.width.saturating_sub(1);
    for (i, rule) in resolver
        .theme
        .rules
        .iter()
        .enumerate()
        .skip(state.scroll)
        .take(visible)
    {
        let row = (i - state.scroll) as u16;
        let kind = match rule.kind {
            Kind::Folder => "dir ",
            Kind::File => "file",
            Kind::Extension => "ext ",
            Kind::Path => "glob",
        };
        let glyph = if state.ascii {
            if rule.kind == Kind::Folder { "D" } else { "F" }
        } else {
            &rule.icon.glyph
        };
        let selected = i == state.selected;
        let bg = if selected {
            Color::DarkGray
        } else {
            Color::Reset
        };
        let line = Line::from(vec![
            Span::styled(format!(" {kind} "), Style::default().fg(Color::Cyan).bg(bg)),
            Span::styled(
                format!("{glyph} "),
                Style::default().fg(foreground(&rule.icon)).bg(bg),
            ),
            Span::styled(
                rule.pattern.as_str(),
                Style::default().fg(Color::White).bg(bg),
            ),
        ]);
        frame.render_widget(
            Paragraph::new(line),
            Rect {
                x: inner.x,
                y: inner.y + row,
                width: list_width,
                height: 1,
            },
        );
    }
    if inner.width > 0 {
        let (thumb_start, thumb_len) =
            scrollbar_geometry(resolver.theme.rules.len(), visible, state.scroll);
        for row in 0..visible {
            let thumb = row >= thumb_start && row < thumb_start + thumb_len;
            let symbol = if state.ascii {
                if thumb { "#" } else { "|" }
            } else if thumb {
                "█"
            } else {
                "│"
            };
            frame.render_widget(
                Paragraph::new(symbol).style(Style::default().fg(if thumb {
                    Color::Cyan
                } else {
                    Color::DarkGray
                })),
                Rect {
                    x: inner.x + inner.width - 1,
                    y: inner.y + row as u16,
                    width: 1,
                    height: 1,
                },
            );
        }
    }
    frame.render_widget(Paragraph::new(" Tab: switch focus   Ctrl+E: expanded   List: arrows/wheel/drag   Enter/click: preview   Esc: exit"), chunks[3]);
    if state.focus == Focus::Input && input_width > 0 {
        frame.set_cursor_position((
            chunks[0].x + 1 + state.cursor.saturating_sub(scroll).min(input_width - 1) as u16,
            chunks[0].y + 1,
        ));
    }
    Regions {
        input: chunks[0],
        list: inner,
    }
}

fn scrollbar_geometry(len: usize, height: usize, scroll: usize) -> (usize, usize) {
    if height == 0 {
        return (0, 0);
    }
    if len <= height {
        return (0, height);
    }
    let thumb = (height.saturating_mul(height) / len).clamp(1, height);
    let start = scroll.min(len - height) * (height - thumb) / (len - height);
    (start, thumb)
}

fn seek_scrollbar(state: &mut State, len: usize, height: usize, row: usize) {
    if height == 0 || len <= height {
        return;
    }
    let (_, thumb) = scrollbar_geometry(len, height, state.scroll);
    let travel = height - thumb;
    let position = row.saturating_sub(thumb / 2).min(travel);
    state.scroll = if travel == 0 {
        0
    } else {
        position * (len - height) / travel
    };
    state.selected = state.scroll;
}

fn clamp(state: &mut State, len: usize, height: usize) {
    state.selected = state.selected.min(len.saturating_sub(1));
    if state.selected < state.scroll {
        state.scroll = state.selected;
    }
    if state.selected >= state.scroll.saturating_add(height.max(1)) {
        state.scroll = state.selected + 1 - height.max(1);
    }
    state.scroll = state.scroll.min(len.saturating_sub(height));
}

fn select_preview(state: &mut State, resolver: &Resolver) {
    if let Some(rule) = resolver.theme.rules.get(state.selected) {
        state.query = match rule.kind {
            Kind::Extension => format!("sample.{}", rule.pattern),
            Kind::Path => rule.pattern.replace('*', "demo").replace('?', "x"),
            Kind::Folder => format!("{}/", rule.pattern.trim_end_matches(['/', '\\'])),
            _ => rule.pattern.clone(),
        };
        state.cursor = state.query.chars().count();
        state.focus = Focus::Input;
    }
}

fn restore() -> Result<()> {
    execute!(
        io::stdout(),
        crossterm::event::DisableMouseCapture,
        LeaveAlternateScreen
    )?;
    disable_raw_mode()?;
    Ok(())
}

pub fn run(resolver: Resolver, ascii: bool) -> Result<()> {
    enable_raw_mode()?;
    if let Err(err) = execute!(
        io::stdout(),
        EnterAlternateScreen,
        crossterm::event::EnableMouseCapture
    ) {
        disable_raw_mode()?;
        return Err(err.into());
    }
    let result = run_loop(resolver, ascii);
    let cleanup = restore();
    result.and(cleanup)
}

fn run_loop(resolver: Resolver, ascii: bool) -> Result<()> {
    let mut terminal: Terminal<CrosstermBackend<Stdout>> =
        Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut state = State {
        query: "src/".into(),
        cursor: 4,
        focus: Focus::Input,
        expanded: false,
        selected: 0,
        scroll: 0,
        ascii,
    };
    loop {
        let mut regions = Regions {
            input: Rect::default(),
            list: Rect::default(),
        };
        terminal.draw(|f| regions = render(f, &resolver, &state))?;
        let height = usize::from(regions.list.height);
        let len = resolver.theme.rules.len();
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => match (key.code, key.modifiers) {
                (KeyCode::Esc, _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => break,
                (KeyCode::Tab | KeyCode::BackTab, _) => {
                    state.focus = if state.focus == Focus::Input {
                        Focus::List
                    } else {
                        Focus::Input
                    };
                }
                (KeyCode::Char('e'), KeyModifiers::CONTROL) => state.expanded = !state.expanded,
                (KeyCode::Up, _) if state.focus == Focus::List => {
                    state.selected = state.selected.saturating_sub(1)
                }
                (KeyCode::Down, _) if state.focus == Focus::List => {
                    state.selected = state.selected.saturating_add(1)
                }
                (KeyCode::PageUp, _) if state.focus == Focus::List => {
                    state.selected = state.selected.saturating_sub(height)
                }
                (KeyCode::PageDown, _) if state.focus == Focus::List => {
                    state.selected = state.selected.saturating_add(height)
                }
                (KeyCode::Home, _) if state.focus == Focus::List => state.selected = 0,
                (KeyCode::End, _) if state.focus == Focus::List => {
                    state.selected = len.saturating_sub(1)
                }
                (KeyCode::Enter, _) if state.focus == Focus::List => {
                    select_preview(&mut state, &resolver)
                }
                (KeyCode::Home, _) if state.focus == Focus::Input => state.cursor = 0,
                (KeyCode::End, _) if state.focus == Focus::Input => {
                    state.cursor = state.query.chars().count()
                }
                (KeyCode::Left, _) if state.focus == Focus::Input => {
                    state.cursor = state.cursor.saturating_sub(1)
                }
                (KeyCode::Right, _) if state.focus == Focus::Input => {
                    state.cursor = (state.cursor + 1).min(state.query.chars().count())
                }
                (KeyCode::Backspace, _) if state.focus == Focus::Input => {
                    delete_char(&mut state, true)
                }
                (KeyCode::Delete, _) if state.focus == Focus::Input => {
                    delete_char(&mut state, false)
                }
                (KeyCode::Char(c), mods)
                    if state.focus == Focus::Input
                        && !mods.contains(KeyModifiers::CONTROL)
                        && !mods.contains(KeyModifiers::ALT) =>
                {
                    insert_char(&mut state, c);
                }
                _ => {}
            },
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollUp
                    if regions.list.contains((mouse.column, mouse.row).into()) =>
                {
                    state.selected = state.selected.saturating_sub(3)
                }
                MouseEventKind::ScrollDown
                    if regions.list.contains((mouse.column, mouse.row).into()) =>
                {
                    state.selected = state.selected.saturating_add(3)
                }
                MouseEventKind::Down(MouseButton::Left)
                | MouseEventKind::Drag(MouseButton::Left)
                    if regions.list.contains((mouse.column, mouse.row).into()) =>
                {
                    state.focus = Focus::List;
                    if mouse.column == regions.list.x + regions.list.width - 1 {
                        seek_scrollbar(
                            &mut state,
                            len,
                            height,
                            usize::from(mouse.row - regions.list.y),
                        );
                    } else if matches!(mouse.kind, MouseEventKind::Down(_)) {
                        state.selected = (state.scroll + usize::from(mouse.row - regions.list.y))
                            .min(len.saturating_sub(1));
                        select_preview(&mut state, &resolver);
                    }
                }
                MouseEventKind::Down(MouseButton::Left)
                    if regions.input.contains((mouse.column, mouse.row).into()) =>
                {
                    state.focus = Focus::Input;
                    let width = usize::from(regions.input.width.saturating_sub(2));
                    let offset = input_scroll(&state, width);
                    let clicked = usize::from(mouse.column.saturating_sub(regions.input.x + 1));
                    state.cursor = (offset + clicked).min(state.query.chars().count());
                }
                _ => {}
            },
            _ => {}
        }
        clamp(&mut state, len, height);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trailing_separator_only_marks_folder() {
        assert!(is_directory("src/"));
        assert!(is_directory("src\\"));
        assert!(!is_directory("src/a.exe"));
        assert!(!is_directory("src\\a.exe"));
        assert!(!is_directory("src"));
    }

    #[test]
    fn scrollbar_reaches_full_list() {
        assert_eq!(scrollbar_geometry(100, 10, 0).0, 0);
        assert_eq!(scrollbar_geometry(100, 10, 90), (9, 1));
        assert_eq!(scrollbar_geometry(4, 10, 0), (0, 10));
    }

    #[test]
    fn unicode_input_edits_are_boundary_safe() {
        let mut state = State {
            query: "é/a".into(),
            cursor: 1,
            focus: Focus::Input,
            expanded: false,
            selected: 0,
            scroll: 0,
            ascii: false,
        };
        insert_char(&mut state, '🚀');
        assert_eq!(state.query, "é🚀/a");
        delete_char(&mut state, true);
        assert_eq!(state.query, "é/a");
        delete_char(&mut state, false);
        assert_eq!(state.query, "éa");
    }
}
