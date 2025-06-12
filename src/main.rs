pub use app::App;

pub mod app;

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();

    // Enable mouse support
    crossterm::execute!(std::io::stdout(), crossterm::event::EnableMouseCapture)?;

    let result = App::new().run(terminal);

    // Disable mouse support before exiting
    crossterm::execute!(std::io::stdout(), crossterm::event::DisableMouseCapture)?;

    ratatui::restore();
    result
}
