use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
    symbols,
    widgets::{Axis, Block, Chart, Clear, Dataset, GraphType, Row, Table, TableState},
    DefaultTerminal, Frame,
};
use sysinfo::ProcessesToUpdate;
use tui_textarea::TextArea;

#[derive(Debug, Default)]
pub struct App {
    running: bool,
    system: sysinfo::System,
    cpu: Vec<(f64, f64)>,
    table_state: TableState,
    textarea: TextArea<'static>,
    search: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            running: true,
            system: sysinfo::System::new_all(),
            cpu: vec![],
            table_state: TableState::default(),
            textarea: {
                let mut textarea = TextArea::default();
                textarea.set_block(Block::bordered().title("Search"));
                textarea
            },
            search: false,
        }
    }

    pub fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        self.running = true;
        self.table_state.select(Some(0));
        while self.running {
            terminal.draw(|frame| {
                if frame.count() % 60 == 0 {
                    self.system.refresh_processes(ProcessesToUpdate::All, true);
                }
                self.system.refresh_cpu_all();
                self.cpu
                    .push((frame.count() as f64, self.system.global_cpu_usage() as f64));
                self.draw(frame)
            })?;
            self.handle_crossterm_events()?;
        }
        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame) {
        let [top, second, third] = Layout::vertical([
            Constraint::Percentage((25)),
            Constraint::Fill(1),
            Constraint::Fill(1),
        ])
        .areas(frame.area());

        let [left, right] =
            Layout::horizontal([Constraint::Percentage((50)), Constraint::Percentage((50))])
                .areas(second);

        let datasets = vec![
            // Scatter chart
            Dataset::default()
                .name("data1")
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().cyan())
                .data(&self.cpu),
        ];
        let x_axis = Axis::default()
            .bounds([0f64, self.cpu.len() as f64])
            .style(Style::default().cyan());
        let y_axis = Axis::default()
            .bounds([0f64, 100f64])
            .style(Style::default().cyan());
        let chart = Chart::new(datasets)
            .block(Block::bordered().title("CPU"))
            .x_axis(x_axis)
            .y_axis(y_axis);

        frame.render_widget(Block::bordered(), left);
        frame.render_widget(Block::bordered(), right);

        frame.render_widget(chart, top);
        //frame.render_widget(Block::bordered(), second);

        self.render_processes(frame, third);

        if self.search {
            self.render_search(frame, third);
        }
    }

    fn render_processes(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let mut rows: Vec<_> = vec![];
        for (pid, process) in self.system.processes() {
            let name = process.name().to_string_lossy().to_string();
            let cpu = process.cpu_usage();
            let row = vec![pid.to_string(), name, cpu.to_string()];
            rows.push(row);
        }

        rows.sort_by(|a, b| {
            let a = a[2].parse::<f32>().unwrap_or(0.0);
            let b = b[2].parse::<f32>().unwrap_or(0.0);
            b.partial_cmp(&a).unwrap()
        });

        let text = self.textarea.lines().first().unwrap();
        rows.retain(|row| {
            row.iter()
                .any(|cell| cell.to_lowercase().contains(&text.to_lowercase()))
        });

        let table = Table::new(
            rows.into_iter().map(Row::new).collect::<Vec<Row>>(),
            [
                Constraint::Max(10),
                Constraint::Fill(1),
                Constraint::Fill(1),
            ],
        )
        .row_highlight_style(Style::default().bg(Color::DarkGray))
        .highlight_symbol(">>")
        .block(Block::bordered().title("Processes"))
        .header(Row::new(vec!["PID", "Name", "CPU"]).style(Style::default().bold()));

        frame.render_stateful_widget(table, area, &mut self.table_state);
    }

    fn render_search(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let search_area = Rect {
            x: area.x + 1,
            y: area.y + 1,
            width: area.width - 2,
            height: 3,
        };
        frame.render_widget(Clear, search_area);
        frame.render_widget(&self.textarea, search_area);
    }

    fn handle_crossterm_events(&mut self) -> Result<()> {
        if event::poll(std::time::Duration::from_millis(16))? {
            match event::read()? {
                // it's important to check KeyEventKind::Press to avoid handling key release events
                Event::Key(key) if key.kind == KeyEventKind::Press => self.on_key_event(key),
                Event::Mouse(_) => {}
                Event::Resize(_, _) => {}
                _ => {}
            }
        }
        Ok(())
    }

    fn on_key_event(&mut self, key: KeyEvent) {
        if self.search {
            self.textarea.input(key);
        }
        match (key.modifiers, key.code) {
            (_, KeyCode::Esc | KeyCode::Char('q'))
            | (KeyModifiers::CONTROL, KeyCode::Char('c') | KeyCode::Char('C')) => self.quit(),

            (_, KeyCode::Char('j')) => {
                self.table_state.select_next();
            }
            (_, KeyCode::Char('k')) => {
                self.table_state.select_previous();
            }
            (_, KeyCode::Char('s')) => {
                self.search = !self.search;
            }
            _ => {}
        }
    }

    /// Set running to false to quit the application.
    fn quit(&mut self) {
        self.running = false;
    }
}
