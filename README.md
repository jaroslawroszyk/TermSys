# Terminal Process Manager

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Rust](https://img.shields.io/badge/rust-stable-blue)
![Status](https://img.shields.io/badge/status-active-green)

A powerful, interactive, and cross-platform terminal-based process manager written in Rust.  
Inspired by tools like `htop`, but focused on responsiveness, safety, and rich UI with mouse support and keyboard-driven control.

---

## Features

- **Live Search** – filter processes by name, PID, or CPU usage
- **Process Table** – sorted by CPU usage, auto-refreshed
- **Kill Process**
  - Select from list and send `SIGTERM` or `SIGKILL`
  - Enter PID manually
- **Mouse Support**
  - Click to select a process
  - Scroll with the mouse wheel
- **Process Details Panel**
  - Executable path, command, working directory
  - Memory usage, disk I/O, start time
- **System Info Panel** – memory, swap, uptime
- **Keyboard Shortcuts** for fast interaction

## Getting Started

### Prerequisites

- Rust (stable) – install via [rustup.rs](https://rustup.rs)
- Cargo

### Build and Run

```bash
git clone https://github.com/your-username/terminal-process-manager.git
cd terminal-process-manager
cargo run --release
````

### Optional

Install globally:

```bash
cargo install --path .
```

---

## 🎮 Keyboard Shortcuts

| Key         | Action                                    |
| ----------- | ----------------------------------------- |
| `q` / `Esc` | Quit application                          |
| `j` / `k`   | Move selection down / up                  |
| `s`         | Toggle search bar                         |
| `Enter`     | Show process detail panel                 |
| `d`         | Kill selected process                     |
| `p`         | Kill process by entering PID              |
| `1` / `2`   | Select signal in kill modal (TERM / KILL) |
| `Esc`       | Close modal, exit detail or search view   |

---


## Dependencies

- [ratatui](https://docs.rs/ratatui) – TUI rendering
- [crossterm](https://docs.rs/crossterm) – terminal input/output
- [sysinfo](https://docs.rs/sysinfo) – system process info
- [tui-textarea](https://crates.io/crates/tui-textarea) – multiline input
- [color-eyre](https://crates.io/crates/color-eyre) – error handling

---

##  License

This project is licensed under the MIT License. See [`LICENSE`](LICENSE) for details.

## Acknowledgements

- `htop` – for UI inspiration
- `ratatui` community – for great terminal UI ecosystem
