use color_eyre::Result;
use crossterm::event::{self, KeyCode, KeyEvent};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::palette::tailwind::{BLUE, SLATE};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::symbols;
use ratatui::text::Line;
use ratatui::widgets::{
    Block, Borders, HighlightSpacing, List, ListItem, ListState, Paragraph, StatefulWidget, Tabs,
    Widget, Wrap,
};
use ratatui::{DefaultTerminal, Frame};
use std::io::{self, IsTerminal, Read};
use std::process::Command;

use cream::{ChunkPathPairs, Output, PathMatch};

const HEADER_STYLE: Style = Style::new().fg(SLATE.c100).bg(BLUE.c800);
const NORMAL_ROW_BG: Color = SLATE.c950;
const ALT_ROW_BG_COLOR: Color = SLATE.c900;
const SELECTED_STYLE: Style = Style::new().bg(SLATE.c800).add_modifier(Modifier::BOLD);

fn main() -> Result<()> {
    color_eyre::install()?;

    let mut i_stdin = String::new();
    let mut i_stdout = String::new();
    let mut i_stderr = String::new();

    let piped = !io::stdin().is_terminal();
    if piped {
        io::stdin().read_to_string(&mut i_stdin).unwrap();
    } else {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let Some((command, command_args)) = args.split_first() else {
            eprintln!("usage: oreo [--] <command> [args...]");
            std::process::exit(2);
        };

        let output = match Command::new(command).args(command_args).output() {
            Ok(output) => output,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                eprintln!("oreo: command not found: {command}");
                std::process::exit(127);
            }
            Err(e) => return Err(e.into()),
        };

        i_stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        i_stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    }

    let output = Output {
        o_stdin: ChunkPathPairs::new(&i_stdin),
        o_stdout: ChunkPathPairs::new(&i_stdout),
        o_stderr: ChunkPathPairs::new(&i_stderr),
    };

    let start_tab = if piped {
        TabKind::Stdin
    } else {
        TabKind::Stdout
    };
    ratatui::run(|terminal| App::new(output, start_tab).run(terminal))
}

struct App<'out> {
    should_exit: bool,
    current_tab: TabKind,
    output: Output<'out>,
    // One ListState per tab so each tab keeps its own selection.
    list_states: [ListState; 3],
}

#[derive(Clone, Copy)]
enum TabKind {
    Stdin,
    Stdout,
    Stderr,
}

impl TabKind {
    const ALL: [Self; 3] = [Self::Stdin, Self::Stdout, Self::Stderr];

    fn index(self) -> usize {
        self as usize
    }

    fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    fn prev(self) -> Self {
        Self::ALL[(self.index() + Self::ALL.len() - 1) % Self::ALL.len()]
    }

    fn title(self) -> &'static str {
        match self {
            Self::Stdin => "stdin",
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }
}

impl<'out> App<'out> {
    fn new(output: Output<'out>, start_tab: TabKind) -> Self {
        Self {
            should_exit: false,
            current_tab: start_tab,
            output,
            list_states: std::array::from_fn(|_| ListState::default()),
        }
    }

    fn current_tab_items<'a>(&'a self) -> &'a ChunkPathPairs<'out> {
        match self.current_tab {
            TabKind::Stdin => &self.output.o_stdin,
            TabKind::Stdout => &self.output.o_stdout,
            TabKind::Stderr => &self.output.o_stderr,
        }
    }

    fn current_state(&mut self) -> &mut ListState {
        &mut self.list_states[self.current_tab.index()]
    }
}

impl<'out> App<'out> {
    fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.should_exit {
            terminal.draw(|frame| self.render(frame))?;
            // terminal.draw(|frame| frame.render_widget(&mut self, frame.area()))?;
            if let Some(key) = event::read()?.as_key_press_event() {
                self.handle_key(key);
            }
        }
        Ok(())
    }

    fn render(&mut self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_exit = true,
            KeyCode::Char('h') | KeyCode::Left => self.current_tab = self.current_tab.prev(),
            KeyCode::Char('l') | KeyCode::Right => self.current_tab = self.current_tab.next(),
            KeyCode::Char('j') | KeyCode::Down => self.current_state().select_next(),
            KeyCode::Char('k') | KeyCode::Up => self.current_state().select_previous(),
            KeyCode::Char('g') | KeyCode::Home => self.current_state().select_first(),
            KeyCode::Char('G') | KeyCode::End => self.current_state().select_last(),
            KeyCode::Enter => self.edit_file(),
            _ => {}
        }
    }
}

impl<'out> Widget for &mut App<'out> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let [tabs_area, list_area, list_extra_area, footer_area] =
            area.layout(&Layout::vertical([
                Constraint::Length(1),
                Constraint::Fill(3),
                Constraint::Fill(1),
                Constraint::Length(1),
            ]));

        self.render_tabs(tabs_area, buf);
        self.render_list(list_area, buf);
        self.render_list_content(list_area.width, list_extra_area, buf);
        self.render_footer(footer_area, list_area.width, buf);
    }
}

/// Rendering logic for the app
impl<'out> App<'out> {
    fn render_tabs(&self, area: Rect, buf: &mut Buffer) {
        Tabs::new(TabKind::ALL.map(|t| t.title()))
            .select(self.current_tab.index())
            .style(SLATE.c200)
            .highlight_style(HEADER_STYLE)
            .divider("|")
            .render(area, buf);
    }

    fn render_footer(&mut self, area: Rect, list_width: u16, buf: &mut Buffer) {
        Paragraph::new(format!(
            "←/→ switch tab · ↑/↓ move · g/G top/bottom · Enter open · q quit, {list_width}"
        ))
        .centered()
        .render(area, buf);
    }

    fn render_list(&mut self, area: Rect, buf: &mut Buffer) {
        let title = format!("Matches for {}", self.current_tab.title());
        let block = Block::new()
            .title(Line::raw(title).centered())
            .borders(Borders::TOP)
            .border_set(symbols::border::EMPTY)
            .border_style(HEADER_STYLE)
            .bg(NORMAL_ROW_BG);

        let items: Vec<ListItem> = self
            .current_tab_items()
            .pairs
            .iter()
            .enumerate()
            .map(|(idx, packed)| {
                ListItem::new(format_item(&packed.path_match))
                    .fg(SLATE.c200)
                    .bg(alternate_colors(idx))
            })
            .collect();

        let list = List::new(items)
            .block(block)
            .highlight_style(SELECTED_STYLE)
            .highlight_symbol(">")
            .highlight_spacing(HighlightSpacing::Always);

        StatefulWidget::render(list, area, buf, self.current_state());
    }

    fn render_list_content(&mut self, list_width: u16, area: Rect, buf: &mut Buffer) {
        let Some(idx) = self.current_state().selected() else {
            return;
        };

        let info = &self.current_tab_items().pairs[idx].chunk;

        if info.chars().count() <= list_width as usize {
            return;
        }

        Paragraph::new(*info)
            .block(Block::new().title("Full text "))
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }
}

impl<'out> App<'out> {
    fn edit_file(&mut self) {
        // TODO: open the selected match in $EDITOR
        self.current_state().select_first();
    }
}
fn format_item(value: &PathMatch) -> String {
    let empty_or_not_here_i_come = |n: Option<u32>| n.map_or(" ".to_string(), |n| format!(": {n}"));

    format!(
        "{}{}{}",
        value.path,
        empty_or_not_here_i_come(value.line_num),
        empty_or_not_here_i_come(value.col_num),
    )
}

const fn alternate_colors(i: usize) -> Color {
    if i.is_multiple_of(2) {
        NORMAL_ROW_BG
    } else {
        ALT_ROW_BG_COLOR
    }
}
