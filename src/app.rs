use color_eyre::Result;
use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
    widgets::{Axis, Block, Chart, Clear, Dataset, GraphType, Row, Table, TableState},
    DefaultTerminal, Frame,
};
use sysinfo::Signal;
use sysinfo::{ProcessesToUpdate, System};
use tui_textarea::TextArea;
use users::get_user_by_uid;

#[derive(Debug, Default)]
pub struct App {
    running: bool,
    system: sysinfo::System,
    cpu: Vec<(f64, f64)>,
    table_state: TableState,
    textarea: TextArea<'static>,
    search: bool,
    kill_modal: bool,
    kill_pid: Option<sysinfo::Pid>,
    kill_by_pid_modal: bool,
    kill_by_pid_input: String,
    process_list_area: Rect,
    details_panel: bool,
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
                // textarea.set_block(Block::bordered().title("Search"));
                textarea.set_block(
                    Block::default()
                        .borders(ratatui::widgets::Borders::ALL)
                        .title("Search (active)")
                        .style(Style::default().fg(Color::Cyan)),
                );
                textarea
            },
            search: false,
            kill_modal: false,
            kill_pid: None,
            kill_by_pid_modal: false,
            kill_by_pid_input: String::new(),
            process_list_area: Rect::default(),
            details_panel: false,
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
        // Shift layout down by 1 row, and use the first area for the CPU chart
        let [cpu_bar, second, third, footer] = Layout::vertical([
            Constraint::Length(8),
            Constraint::Percentage(25),
            Constraint::Fill(1),
            Constraint::Length(3),
        ])
        .areas(frame.area());

        // Draw CPU usage line chart at the very top
        let max_points = cpu_bar.width.min(120) as usize;
        if self.cpu.len() > max_points {
            self.cpu.drain(0..self.cpu.len() - max_points);
        }
        let cpu_data: Vec<(f64, f64)> = self.cpu.to_vec();
        let chart = Chart::new(vec![Dataset::default()
            .name("CPU Usage")
            .marker(ratatui::symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::Cyan))
            .data(&cpu_data)])
        .block(Block::bordered().title("CPU Usage (%)"))
        .x_axis(Axis::default().bounds([
            cpu_data.first().map(|(x, _)| *x).unwrap_or(0.0),
            cpu_data.last().map(|(x, _)| *x).unwrap_or(1.0),
        ]))
        .y_axis(Axis::default().bounds([0.0, 100.0]));
        frame.render_widget(chart, cpu_bar);

        let [left, right] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                .areas(second);

        // Left: process details
        self.render_process_details(frame, left);

        // Right: show some system info
        let total_mem_gb = self.system.total_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
        let used_mem_gb = self.system.used_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
        let total_swap_gb = self.system.total_swap() as f64 / 1024.0 / 1024.0 / 1024.0;
        let used_swap_gb = self.system.used_swap() as f64 / 1024.0 / 1024.0 / 1024.0;
        let uptime = System::uptime();
        let days = uptime / 86400;
        let hours = (uptime % 86400) / 3600;
        let minutes = (uptime % 3600) / 60;
        let seconds = uptime % 60;
        let uptime_str = format!("{:02}d {:02}h {:02}m {:02}s", days, hours, minutes, seconds);
        let cpu_usage = self.system.global_cpu_usage();
        let sys_info = format!(
            "System Information\n\
            ───────────────────────────────\n\
            CPU Usage    : {:>6.2} %\n\
            Total Memory : {:>8.2} GB\n\
            Used Memory  : {:>8.2} GB\n\
            Total Swap   : {:>8.2} GB\n\
            Used Swap    : {:>8.2} GB\n\
            Uptime       : {}",
            cpu_usage, total_mem_gb, used_mem_gb, total_swap_gb, used_swap_gb, uptime_str
        );
        let info_paragraph = ratatui::widgets::Paragraph::new(sys_info)
            .block(Block::bordered().title("System Info"));
        frame.render_widget(info_paragraph, right);

        // Make process table fill all available space below the panels
        self.render_processes(frame, third);

        if self.search {
            self.render_search(frame, third);
        }

        if self.kill_modal {
            self.render_kill_modal(frame, third);
        }

        if self.kill_by_pid_modal {
            self.render_kill_by_pid_modal(frame, third);
        }

        if self.details_panel {
            self.render_details_panel(frame);
        }

        self.render_footer(frame, footer);

        // Store the process list area for mouse handling
        self.process_list_area = third;
    }

    fn render_process_details(&mut self, frame: &mut Frame<'_>, area: Rect) {
        // Show details of the selected process
        let mut text = String::from("No process selected");
        if let Some(selected) = self.table_state.selected() {
            let processes: Vec<_> = self.system.processes().iter().collect();
            if selected < processes.len() {
                let (_pid, process) = processes[selected];
                text = format!(
                    "PID: {}\nName: {:?}\nCPU: {:.2}%\nMemory: {:.2} MB\nStatus: {:?}",
                    _pid,
                    process.name(),
                    process.cpu_usage(),
                    process.memory() as f64 / 1024.0 / 1024.0,
                    process.status()
                );
            }
        }
        let paragraph = ratatui::widgets::Paragraph::new(text)
            .block(Block::bordered().title("Process Details"));
        frame.render_widget(paragraph, area);
    }

    fn render_footer(&self, frame: &mut Frame<'_>, area: Rect) {
        use ratatui::widgets::Paragraph;
        let help =
            "[q/Esc] Quit  [s] Toggle Search  [j/k] Move  [d] Kill  [p] Kill by PID  [Enter] Details  [In Search: Esc] Exit Search  [In Details: Esc] Close";
        let paragraph = Paragraph::new(help).block(Block::bordered().title("Help"));
        frame.render_widget(paragraph, area);
    }

    fn render_processes(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let mut rows: Vec<_> = vec![];
        for (pid, process) in self.system.processes() {
            let name = process.name().to_string_lossy().to_string();
            let user = process
                .user_id()
                .and_then(|uid| get_user_by_uid(**uid))
                .map(|u| u.name().to_string_lossy().to_string())
                .unwrap_or_default();
            let cpu = format!("{:.1}%", process.cpu_usage());
            let mem_mb = format!("{:.1}", process.memory() as f64 / 1024.0 / 1024.0);
            let row = vec![pid.to_string(), name, user, cpu, mem_mb];
            // Create a row with appropriate styling based on process status
            let style = match process.status() {
                sysinfo::ProcessStatus::Run => Style::default().fg(Color::Green),
                sysinfo::ProcessStatus::Sleep => Style::default().fg(Color::Yellow),
                sysinfo::ProcessStatus::Zombie => Style::default().fg(Color::Red),
                _ => Style::default(),
            };
            rows.push((row, style));
        }

        rows.sort_by(|a, b| {
            let a_cpu = a.0[3].replace('%', "").parse::<f32>().unwrap_or(0.0);
            let b_cpu = b.0[3].replace('%', "").parse::<f32>().unwrap_or(0.0);
            b_cpu.partial_cmp(&a_cpu).unwrap()
        });

        let text = self.textarea.lines().first().unwrap();
        rows.retain(|(row, _)| {
            row.iter()
                .any(|cell| cell.to_lowercase().contains(&text.to_lowercase()))
        });

        let table = Table::new(
            rows.into_iter()
                .map(|(row, style)| Row::new(row).style(style))
                .collect::<Vec<Row>>(),
            [
                Constraint::Max(10),
                Constraint::Fill(1),
                Constraint::Max(10),
                Constraint::Max(8),
                Constraint::Max(10),
            ],
        )
        .row_highlight_style(Style::default().bg(Color::DarkGray))
        .highlight_symbol(">>")
        .block(Block::bordered().title("Processes"))
        .header(
            Row::new(vec!["PID", "Name", "User", "CPU%", "MemMB"]).style(Style::default().bold()),
        );

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

    fn render_kill_modal(&self, frame: &mut Frame<'_>, area: Rect) {
        use ratatui::widgets::Paragraph;
        let text =
            "Select signal to send:\n[1] SIGTERM (graceful)\n[2] SIGKILL (force)\n[Esc] Cancel";
        let modal_area = Rect {
            x: area.x + area.width / 4,
            y: area.y + area.height / 4,
            width: area.width / 2,
            height: 7,
        };
        frame.render_widget(Clear, modal_area);
        let paragraph = Paragraph::new(text).block(Block::bordered().title("Kill process"));
        frame.render_widget(paragraph, modal_area);
    }

    fn render_kill_by_pid_modal(&self, frame: &mut Frame<'_>, area: Rect) {
        use ratatui::widgets::Paragraph;
        let text = format!(
            "Enter PID to kill with SIGKILL:\n[{}]\n[Enter] Kill   [Esc] Cancel",
            self.kill_by_pid_input
        );
        let modal_area = Rect {
            x: area.x + area.width / 4,
            y: area.y + area.height / 4,
            width: area.width / 2,
            height: 6,
        };
        frame.render_widget(Clear, modal_area);
        let paragraph = Paragraph::new(text).block(Block::bordered().title("Kill by PID"));
        frame.render_widget(paragraph, modal_area);
    }

    fn render_details_panel(&self, frame: &mut Frame) {
        if let Some(selected) = self.table_state.selected() {
            let processes: Vec<_> = self.system.processes().iter().collect();
            if selected < processes.len() {
                let (pid, process) = processes[selected];

                // Get detailed process information
                let exe = process
                    .exe()
                    .map(|p| format!("{:?}", p))
                    .unwrap_or_else(|| "Unknown".to_string());
                let cmd = process
                    .cmd()
                    .iter()
                    .map(|s| s.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(" ");
                let cwd = process
                    .cwd()
                    .map(|p| format!("{:?}", p))
                    .unwrap_or_else(|| "Unknown".to_string());
                let disk_usage = process.disk_usage();
                let memory = process.memory();
                let virtual_memory = process.virtual_memory();
                let start_time = format!("{:?}", process.start_time());
                let run_time = format!("{:?}", process.run_time());
                let status = format!("{:?}", process.status());

                let details = format!(
                    "Process Details for PID {}\n\n\
                    Executable: {}\n\
                    Command: {}\n\
                    Working Directory: {}\n\
                    Status: {}\n\
                    Start Time: {}\n\
                    Run Time: {}s\n\n\
                    Memory Usage:\n\
                    - Physical: {:.2} MB\n\
                    - Virtual: {:.2} MB\n\
                    - Read: {:.2} MB\n\
                    - Written: {:.2} MB",
                    pid,
                    exe,
                    cmd,
                    cwd,
                    status,
                    start_time,
                    run_time,
                    memory as f64 / 1024.0 / 1024.0,
                    virtual_memory as f64 / 1024.0 / 1024.0,
                    disk_usage.read_bytes as f64 / 1024.0 / 1024.0,
                    disk_usage.written_bytes as f64 / 1024.0 / 1024.0
                );

                // Create a panel that takes up 80% of the screen width and height
                let panel_width = (frame.area().width as f32 * 0.8) as u16;
                let panel_height = (frame.area().height as f32 * 0.8) as u16;
                let panel_x = (frame.area().width - panel_width) / 2;
                let panel_y = (frame.area().height - panel_height) / 2;

                let panel_area = Rect::new(panel_x, panel_y, panel_width, panel_height);

                // Clear the area and render the panel
                frame.render_widget(Clear, panel_area);
                let paragraph = ratatui::widgets::Paragraph::new(details)
                    .block(Block::bordered().title("Process Details (Press Esc to close)"));
                frame.render_widget(paragraph, panel_area);
            }
        }
    }

    fn handle_crossterm_events(&mut self) -> Result<()> {
        if event::poll(std::time::Duration::from_millis(16))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => self.on_key_event(key),
                Event::Mouse(mouse) => self.on_mouse_event(mouse),
                Event::Resize(_, _) => {}
                _ => {}
            }
        }
        Ok(())
    }

    fn on_key_event(&mut self, key: KeyEvent) {
        if self.details_panel {
            if key.code == KeyCode::Esc {
                self.details_panel = false;
            }
            return;
        }

        if self.kill_by_pid_modal {
            match key.code {
                KeyCode::Esc => {
                    self.kill_by_pid_modal = false;
                    self.kill_by_pid_input.clear();
                }
                KeyCode::Enter => {
                    self.try_kill_by_pid();
                    self.kill_by_pid_modal = false;
                    self.kill_by_pid_input.clear();
                }
                KeyCode::Char(c) if c.is_ascii_digit() => {
                    self.kill_by_pid_input.push(c);
                }
                KeyCode::Backspace => {
                    self.kill_by_pid_input.pop();
                }
                _ => {}
            }
            return;
        }
        if self.kill_modal {
            match key.code {
                KeyCode::Char('1') => self.send_signal(Signal::Term),
                KeyCode::Char('2') => self.send_signal(Signal::Kill),
                KeyCode::Esc => {
                    self.kill_modal = false;
                    self.kill_pid = None;
                }
                _ => {}
            }
            return;
        }
        if self.search {
            if key.code == KeyCode::Esc {
                self.search = false;
            } else {
                match key.code {
                    KeyCode::Enter => {
                        self.search = false;
                    }
                    _ => {
                        self.textarea.input(key);
                    }
                }
            }
            return;
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
            (_, KeyCode::Char('d')) => {
                self.prepare_kill_modal();
            }
            (_, KeyCode::Char('p')) => {
                self.kill_by_pid_modal = true;
                self.kill_by_pid_input.clear();
            }
            (_, KeyCode::Enter) => {
                self.details_panel = true;
            }
            _ => {}
        }
    }

    fn on_mouse_event(&mut self, mouse: MouseEvent) {
        if self.search || self.kill_modal || self.kill_by_pid_modal {
            return;
        }

        match mouse.kind {
            MouseEventKind::ScrollUp => {
                if let Some(selected) = self.table_state.selected() {
                    if selected > 0 {
                        self.table_state.select(Some(selected - 1));
                    }
                }
            }
            MouseEventKind::ScrollDown => {
                if let Some(selected) = self.table_state.selected() {
                    let processes: Vec<_> = self.system.processes().iter().collect();
                    if selected < processes.len() - 1 {
                        self.table_state.select(Some(selected + 1));
                    }
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                // Get the filtered processes first
                let mut rows: Vec<(sysinfo::Pid, &sysinfo::Process)> = vec![];
                for (pid, process) in self.system.processes() {
                    rows.push((*pid, process));
                }
                rows.sort_by(|a, b| {
                    let a_cpu = a.1.cpu_usage();
                    let b_cpu = b.1.cpu_usage();
                    b_cpu.partial_cmp(&a_cpu).unwrap()
                });
                let text = self.textarea.lines().first().unwrap();
                let filtered: Vec<_> = rows
                    .into_iter()
                    .filter(|(_pid, process)| {
                        let name = process.name().to_string_lossy().to_string();
                        let cpu = process.cpu_usage().to_string();
                        let pid = process.pid().to_string();
                        [pid, name, cpu]
                            .iter()
                            .any(|cell| cell.to_lowercase().contains(&text.to_lowercase()))
                    })
                    .collect();

                // Check if click is within the process list area
                if mouse.column >= self.process_list_area.x && mouse.column < self.process_list_area.x + self.process_list_area.width
                    && mouse.row >= self.process_list_area.y + 2  // Skip header and border
                    && mouse.row < self.process_list_area.y + self.process_list_area.height
                {
                    // Calculate which row was clicked
                    let clicked_row = (mouse.row - (self.process_list_area.y + 2)) as usize;
                    if clicked_row < filtered.len() {
                        self.table_state.select(Some(clicked_row));
                    }
                }
            }
            _ => {}
        }
    }

    fn prepare_kill_modal(&mut self) {
        // Build the same filtered/visible process list as in render_processes
        let mut rows: Vec<(sysinfo::Pid, &sysinfo::Process)> = vec![];
        for (pid, process) in self.system.processes() {
            rows.push((*pid, process));
        }
        rows.sort_by(|a, b| {
            let a_cpu = a.1.cpu_usage();
            let b_cpu = b.1.cpu_usage();
            b_cpu.partial_cmp(&a_cpu).unwrap()
        });
        let text = self.textarea.lines().first().unwrap();
        let filtered: Vec<_> = rows
            .into_iter()
            .filter(|(_pid, process)| {
                let name = process.name().to_string_lossy().to_string();
                let cpu = process.cpu_usage().to_string();
                let pid = process.pid().to_string();
                [pid, name, cpu]
                    .iter()
                    .any(|cell| cell.to_lowercase().contains(&text.to_lowercase()))
            })
            .collect();
        if let Some(selected) = self.table_state.selected() {
            if selected < filtered.len() {
                let (pid, _process) = filtered[selected];
                self.kill_modal = true;
                self.kill_pid = Some(pid);
            }
        }
    }

    fn send_signal(&mut self, sig: Signal) {
        if let Some(pid) = self.kill_pid {
            for (proc_pid, process) in self.system.processes() {
                if *proc_pid == pid {
                    let _ = process.kill_with(sig);
                }
            }
        }
        self.kill_modal = false;
        self.kill_pid = None;
    }

    fn try_kill_by_pid(&mut self) {
        if let Ok(pid_num) = self.kill_by_pid_input.parse::<u32>() {
            let pid = sysinfo::Pid::from_u32(pid_num);
            if let Some(process) = self.system.process(pid) {
                let _ = process.kill_with(Signal::Kill);
            }
        }
    }

    fn quit(&mut self) {
        self.running = false;
    }
}
