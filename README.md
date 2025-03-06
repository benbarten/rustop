# rustop

A lightweight, terminal-based system monitor for macOS written in Rust.

<img width="403" alt="image" src="https://github.com/user-attachments/assets/20733310-f295-465a-8d3c-1da7abbfd454" />


## Overview

Rustop is a terminal-based process viewer and system monitor inspired by the classic Unix `top` command. It provides real-time information about running processes, including CPU usage and memory consumption, in a clean and easy-to-read terminal interface.

## Features

- Real-time process monitoring
- CPU usage percentage display
- Memory usage tracking
- Clean terminal UI with color-coded output
- Automatic sorting by CPU usage
- Responsive terminal display
- Customizable refresh rate
- Sorting by CPU usage, memory usage, or PID
- Filtering processes by name or user
- Option to hide kernel processes
- Human-readable memory format
- Non-interactive mode for scripting

## Requirements

- macOS (uses macOS-specific libraries for process information)
- Rust and Cargo installed

## Installation

Clone this repository:

```bash
git clone https://github.com/yourusername/rustop.git
cd rustop
```

Build the project:

```bash
cargo build --release
```

The compiled binary will be available at `target/release/rustop`.

## Usage

```bash
# Basic usage
cargo run --release

# Show help
cargo run --release -- --help

# Sort by memory usage
cargo run --release -- --sort-by memory

# Show only top 10 processes
cargo run --release -- --top 10

# Filter processes by name (case-insensitive)
cargo run --release -- --filter chrome

# Show only processes owned by a specific user
cargo run --release -- --user yourusername

# Hide kernel processes
cargo run --release -- --no-kernel

# Display memory in human-readable format
cargo run --release -- --human-readable

# Run once and exit (non-interactive mode)
cargo run --release -- --once

# Combine multiple options
cargo run --release -- --sort-by memory --top 5 --human-readable --refresh-rate 2.5
```

## How It Works

Rustop samples process information at regular intervals and calculates the CPU usage based on the difference between samples. It uses:

- `libproc` for macOS process management
- `sysinfo` for system information
- `crossterm` for terminal UI

## Building from Source

```bash
# Clone the repository
git clone https://github.com/yourusername/rustop.git
cd rustop

# Build the project
cargo build --release

# Run the executable
./target/release/rustop
```

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgments

- Inspired by the Unix `top` command
- Built with Rust and its amazing ecosystem

## Command-line Arguments

| Argument | Short | Description |
|----------|-------|-------------|
| `--sort-by` | `-s` | Sort processes by CPU usage, memory usage, or PID (default: cpu) |
| `--refresh-rate` | `-r` | Refresh rate in seconds (default: 1.0) |
| `--top` | `-t` | Show only the top N processes |
| `--filter` | `-f` | Filter processes by name (case-insensitive) |
| `--user` | `-u` | Show only processes owned by the specified user |
| `--no-kernel` | `-k` | Hide kernel processes |
| `--human-readable` | `-H` | Display memory in human-readable format (KB, MB, GB) |
| `--once` | `-o` | Run once and exit (non-interactive mode) |
| `--help` | `-h` | Show help message |
| `--version` | | Show version information | 
