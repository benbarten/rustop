use clap::{Parser, ValueEnum};
use crossterm::{
    cursor::{self, Hide, Show},
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, size},
};
use libproc::libproc::pid_rusage::{RUsageInfoV2, pidrusage};
use libproc::libproc::proc_pid::name;
use libproc::processes;
use std::{cmp::Ordering, io::Error, sync::atomic, thread, time::Duration};
use std::{collections::HashMap, sync::Arc};
use std::{io::ErrorKind, panic};
use std::{
    io::{Stdout, Write, stdout},
    sync::atomic::AtomicBool,
};
use sysinfo::System;

#[derive(Debug, Copy, Clone, PartialEq, Eq, ValueEnum)]
enum SortBy {
    Cpu,
    Memory,
    Pid,
}

#[derive(Parser, Debug)]
#[command(author, version, about = "A simple top-like process viewer written in Rust", long_about = None)]
struct Args {
    /// Sort processes by CPU usage, memory usage, or PID
    #[arg(short, long, value_enum, default_value_t = SortBy::Cpu)]
    sort_by: SortBy,

    /// Refresh rate in seconds
    #[arg(short, long, default_value_t = 1.0)]
    refresh_rate: f64,

    /// Show only the top N processes
    #[arg(short, long)]
    top: Option<usize>,

    /// Filter processes by name (case-insensitive)
    #[arg(short, long)]
    filter: Option<String>,

    /// Show only processes owned by the specified user
    #[arg(short = 'u', long)]
    user: Option<String>,

    /// Hide kernel processes
    #[arg(short = 'k', long)]
    no_kernel: bool,

    /// Display memory in human-readable format (KB, MB, GB)
    #[arg(short = 'H', long)]
    human_readable: bool,
}

struct UsageInfo {
    pid: u32,
    name: String,
    cpu: f64,
    mem: u64,
}

fn sample() -> (HashMap<u32, UsageInfo>, f64) {
    let uptime = System::uptime() as f64;
    let processes_by_type = processes::pids_by_type(processes::ProcFilter::All);
    let mut first_sample = HashMap::new();

    if let Ok(ref pids) = processes_by_type {
        for pid in pids.iter() {
            let proc_name = name(*pid as i32).unwrap_or_else(|_| "Unknown".to_string());

            if let Ok(usage) = pidrusage::<RUsageInfoV2>(*pid as i32) {
                let cpu_time = (usage.ri_system_time + usage.ri_user_time) as f64 / 1_000_000.0;
                first_sample.insert(
                    *pid,
                    UsageInfo {
                        pid: *pid,
                        name: proc_name,
                        cpu: cpu_time,
                        mem: usage.ri_resident_size,
                    },
                );
            }
        }
    }

    (first_sample, uptime)
}

fn stats(num_cpus: f64, sample: (HashMap<u32, UsageInfo>, f64)) -> Vec<UsageInfo> {
    let uptime_t2 = System::uptime() as f64;
    let elapsed_time = (uptime_t2 - sample.1).max(0.01);

    let mut proc_stats: Vec<UsageInfo> = Vec::new();

    for (pid, info) in sample.0.iter() {
        if let Ok(usage) = pidrusage::<RUsageInfoV2>(*pid as i32) {
            let new_cpu_time = (usage.ri_system_time + usage.ri_user_time) as f64 / 1_000_000.0;
            let cpu_usage = ((new_cpu_time - info.cpu) / elapsed_time) * (100.0 / num_cpus);
            proc_stats.push(UsageInfo {
                pid: *pid,
                name: info.name.clone(),
                cpu: cpu_usage,
                mem: usage.ri_resident_size,
            });
        }
    }

    proc_stats
}

/// Format bytes into human-readable format (KB, MB, GB)
fn format_memory(bytes: u64, human_readable: bool) -> String {
    if !human_readable {
        return format!("{}", bytes / 1_000_000); // Default: MB
    }

    const KB: u64 = 1_000;
    const MB: u64 = 1_000_000;
    const GB: u64 = 1_000_000_000;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

fn print(stdout: &mut Stdout, stats: Vec<UsageInfo>, args: &Args) {
    let (_, rows) = size().unwrap_or((0, 0));
    let lines_to_print = match args.top {
        Some(n) => n.min((rows as usize).saturating_sub(2)),
        None => (rows as usize).saturating_sub(2), // Reserve 2 lines for header
    };

    let mem_header = if args.human_readable {
        "Memory"
    } else {
        "Memory (MB)"
    };

    execute!(
        stdout,
        cursor::MoveTo(0, 0),
        Clear(ClearType::All),
        SetForegroundColor(Color::Green),
        Print("\r\n"),
        Print(format!(
            "{:<6} {:<25} {:>10} {:>12}\n",
            "PID", "COMMAND", "CPU (%)", mem_header
        )),
        ResetColor
    )
    .unwrap();

    for stat in stats.iter().take(lines_to_print) {
        execute!(
            stdout,
            SetForegroundColor(Color::DarkYellow),
            Print(format!(
                "\r{:<6} {:<25} {:>9.2}% {:>12}\n",
                stat.pid,
                &stat.name.chars().take(25).collect::<String>(), // Trim long process names
                stat.cpu,
                format_memory(stat.mem, args.human_readable),
            )),
            ResetColor
        )
        .unwrap();
    }

    stdout.flush().unwrap(); // Force immediate terminal update
}

fn setup_terminal(stdout: &mut Stdout) -> Result<(), Box<dyn std::error::Error>> {
    execute!(stdout, EnterAlternateScreen, Hide)?;
    Ok(())
}

fn cleanup_terminal(stdout: &mut Stdout) -> Result<(), Box<dyn std::error::Error>> {
    execute!(stdout, Show, LeaveAlternateScreen)?;
    Ok(())
}

fn main() -> Result<(), Error> {
    let args = Args::parse();
    if args.refresh_rate < 1.0 {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "Refresh rate must be at least 1 second",
        ));
    }

    // Set up panic hook to ensure terminal is restored on panic
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let mut stdout = std::io::stdout();
        let _ = execute!(stdout, Show, LeaveAlternateScreen);
        default_hook(panic_info);
    }));

    let term = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&term))?;
    signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&term))?;

    // Set up a cleanup handler for signals
    let cleanup_on_signal = std::thread::spawn({
        let term = Arc::clone(&term);
        move || {
            while !term.load(atomic::Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(100));
            }
            let mut stdout = std::io::stdout();
            let _ = execute!(stdout, Show, LeaveAlternateScreen);
        }
    });

    let mut sys = System::new_all();
    sys.refresh_all();
    let num_cpus = sys.cpus().len() as f64;

    let mut stdout = stdout();

    let _ = setup_terminal(&mut stdout);

    loop {
        let sample = sample();

        thread::sleep(Duration::from_secs_f64(args.refresh_rate));

        let mut stats = stats(num_cpus, sample);

        // Apply filter if specified
        if let Some(filter) = &args.filter {
            let filter_lower = filter.to_lowercase();
            stats.retain(|stat| stat.name.to_lowercase().contains(&filter_lower));
        }

        // Filter by user if specified
        if let Some(user) = &args.user {
            let user_lower = user.to_lowercase();

            sys.refresh_processes();

            stats.retain(|stat| {
                if let Some(process) = sys.process(sysinfo::Pid::from_u32(stat.pid)) {
                    // Get the user ID if available
                    if let Some(uid) = process.user_id() {
                        return uid.to_string().contains(&user_lower);
                    }
                }
                false
            });
        }

        // Hide kernel processes if requested
        if args.no_kernel {
            // On macOS, kernel processes typically have PIDs < 100
            // This is a simplified approach - in a real implementation, you'd want to use
            // a more reliable method to identify kernel processes
            stats.retain(|stat| {
                // Refresh system info to get process details
                sys.refresh_process(sysinfo::Pid::from_u32(stat.pid));

                if let Some(process) = sys.process(sysinfo::Pid::from_u32(stat.pid)) {
                    // Check if it's a kernel process (simplified approach)
                    return !process.name().starts_with("kernel") && stat.pid >= 100;
                }
                true
            });
        }

        // Sort based on the specified criteria
        match args.sort_by {
            SortBy::Cpu => {
                stats.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap_or(Ordering::Less))
            }
            SortBy::Memory => stats.sort_by(|a, b| b.mem.cmp(&a.mem)),
            SortBy::Pid => stats.sort_by(|a, b| a.pid.cmp(&b.pid)),
        }

        print(&mut stdout, stats, &args);

        sys.refresh_all();

        if term.load(atomic::Ordering::Relaxed) {
            break;
        }
    }

    let _ = cleanup_terminal(&mut stdout);

    // Set the termination flag to true and wait for the cleanup thread to finish
    term.store(true, atomic::Ordering::Relaxed);
    let _ = cleanup_on_signal.join();

    Ok(())
}
