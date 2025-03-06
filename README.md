# Rustop

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

Run the application:

```bash
./target/release/rustop
```

Or, if you've installed it:

```bash
rustop
```

### Controls

- Press `Ctrl+C` to exit the application

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
