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
use serde::{Deserialize, Serialize};
use chrono;

mod config;
use config::Config;

#[derive(Debug, Copy, Clone, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
enum SortBy {
    Cpu,
    Memory,
    Pid,
    StartTime,
}

#[derive(Parser, Debug)]
#[command(author, version, about = "A simple top-like process viewer written in Rust", long_about = None)]
struct Args {
    /// Sort processes by CPU usage, memory usage, PID, or start time
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

    /// Generate a config file with current settings
    #[arg(short = 'g', long)]
    generate_config: bool,
    
    /// Filter processes with CPU usage above this threshold (%)
    #[arg(long)]
    cpu_above: Option<f64>,
    
    /// Filter processes with CPU usage below this threshold (%)
    #[arg(long)]
    cpu_below: Option<f64>,
    
    /// Filter processes with memory usage above this threshold (MB or in bytes if not human-readable)
    #[arg(long)]
    mem_above: Option<u64>,
    
    /// Filter processes with memory usage below this threshold (MB or in bytes if not human-readable)
    #[arg(long)]
    mem_below: Option<u64>,
}

struct UsageInfo {
    pid: u32,
    name: String,
    cpu: f64,
    mem: u64,
    start_time: u64,
}

fn sample() -> (HashMap<u32, UsageInfo>, f64) {
    let uptime = System::uptime() as f64;
    let processes_by_type = processes::pids_by_type(processes::ProcFilter::All);
    let mut first_sample = HashMap::new();
    let mut sys = System::new_all();
    sys.refresh_all();

    if let Ok(ref pids) = processes_by_type {
        for pid in pids.iter() {
            let proc_name = name(*pid as i32).unwrap_or_else(|_| "Unknown".to_string());
            let mut start_time = 0;
            
            // Get process start time using sysinfo
            if let Some(process) = sys.process(sysinfo::Pid::from_u32(*pid)) {
                start_time = process.start_time();
            }

            if let Ok(usage) = pidrusage::<RUsageInfoV2>(*pid as i32) {
                let cpu_time = (usage.ri_system_time + usage.ri_user_time) as f64 / 1_000_000.0;
                first_sample.insert(
                    *pid,
                    UsageInfo {
                        pid: *pid,
                        name: proc_name,
                        cpu: cpu_time,
                        mem: usage.ri_resident_size,
                        start_time,
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
                start_time: info.start_time,
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

/// Format timestamp into a human-readable format (HH:MM:SS)
fn format_time(timestamp: u64) -> String {
    let datetime = chrono::DateTime::from_timestamp(timestamp as i64, 0)
        .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap());
    
    datetime.format("%H:%M:%S").to_string()
}

fn print(stdout: &mut Stdout, stats: Vec<UsageInfo>, args: &Args) {
    let (_, rows) = size().unwrap_or((0, 0));
    let lines_to_print = match args.top {
        Some(n) => n.min((rows as usize).saturating_sub(2)),
        None => (rows as usize).saturating_sub(2), // Reserve 2 lines for header
    };

    let mem_header = if args.human_readable {
        "MEMORY"
    } else {
        "MEMORY (MB)"
    };

    execute!(
        stdout,
        cursor::MoveTo(0, 0),
        Clear(ClearType::All),
        SetForegroundColor(Color::Green),
        Print("\r\n"),
        Print(format!(
            "{:<6} {:<20} {:>10} {:>12} {:>10}\n",
            "PID", "COMMAND", "CPU (%)", mem_header, "START TIME"
        )),
        ResetColor
    )
    .unwrap();

    for stat in stats.iter().take(lines_to_print) {
        execute!(
            stdout,
            SetForegroundColor(Color::DarkYellow),
            Print(format!(
                "\r{:<6} {:<20} {:>10} {:>12} {:>10}\n",
                stat.pid,
                &stat.name.chars().take(20).collect::<String>(), // Trim long process names
                format!("{:.2}%", stat.cpu),
                format_memory(stat.mem, args.human_readable),
                format_time(stat.start_time),
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
    // Load configuration from file
    let config = Config::load().unwrap_or_else(|e| {
        eprintln!("Warning: Failed to load config file: {}", e);
        Config::default()
    });

    // Parse command line arguments
    let mut args = Args::parse();

    // Merge config with command line args (command line takes precedence)
    if let Some(sort_by) = config.sort_by {
        if !std::env::args().any(|arg| arg == "-s" || arg == "--sort-by") {
            args.sort_by = sort_by;
        }
    }

    if let Some(refresh_rate) = config.refresh_rate {
        if !std::env::args().any(|arg| arg == "-r" || arg == "--refresh-rate") {
            args.refresh_rate = refresh_rate;
        }
    }

    if let Some(top) = config.top {
        if !std::env::args().any(|arg| arg == "-t" || arg == "--top") {
            args.top = Some(top);
        }
    }

    if let Some(filter) = config.filter {
        if !std::env::args().any(|arg| arg == "-f" || arg == "--filter") {
            args.filter = Some(filter);
        }
    }

    if let Some(user) = config.user {
        if !std::env::args().any(|arg| arg == "-u" || arg == "--user") {
            args.user = Some(user);
        }
    }

    if let Some(no_kernel) = config.no_kernel {
        if !std::env::args().any(|arg| arg == "-k" || arg == "--no-kernel") {
            args.no_kernel = no_kernel;
        }
    }

    if let Some(human_readable) = config.human_readable {
        if !std::env::args().any(|arg| arg == "-H" || arg == "--human-readable") {
            args.human_readable = human_readable;
        }
    }
    
    // Handle new configuration options
    if let Some(cpu_above) = config.cpu_above {
        if !std::env::args().any(|arg| arg == "--cpu-above") {
            args.cpu_above = Some(cpu_above);
        }
    }
    
    if let Some(cpu_below) = config.cpu_below {
        if !std::env::args().any(|arg| arg == "--cpu-below") {
            args.cpu_below = Some(cpu_below);
        }
    }
    
    if let Some(mem_above) = config.mem_above {
        if !std::env::args().any(|arg| arg == "--mem-above") {
            args.mem_above = Some(mem_above);
        }
    }
    
    if let Some(mem_below) = config.mem_below {
        if !std::env::args().any(|arg| arg == "--mem-below") {
            args.mem_below = Some(mem_below);
        }
    }

    // Create default config file if it doesn't exist
    if let Err(e) = config::ensure_config_file_exists() {
        eprintln!("Warning: Failed to create default config file: {}", e);
    }

    // Handle generate_config flag
    if args.generate_config {
        let config_to_save = Config {
            sort_by: Some(args.sort_by),
            refresh_rate: Some(args.refresh_rate),
            top: args.top,
            filter: args.filter.clone(),
            user: args.user.clone(),
            no_kernel: Some(args.no_kernel),
            human_readable: Some(args.human_readable),
            cpu_above: args.cpu_above,
            cpu_below: args.cpu_below,
            mem_above: args.mem_above,
            mem_below: args.mem_below,
        };
        
        match config_to_save.save() {
            Ok(()) => {
                if let Some(path) = config::get_config_path() {
                    println!("Configuration saved to: {:?}", path);
                } else {
                    println!("Configuration saved successfully.");
                }
                return Ok(());
            }
            Err(e) => {
                return Err(Error::new(
                    ErrorKind::Other,
                    format!("Failed to save configuration: {}", e),
                ));
            }
        }
    }

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
        
        // Refresh system info before applying filters
        sys.refresh_all();

        // Apply all filters using functional programming patterns
        let filters: Vec<Box<dyn Fn(&UsageInfo) -> bool>> = vec![
            // Filter by name if specified
            Box::new({
                let filter_opt = args.filter.clone();
                move |stat: &UsageInfo| -> bool {
                    if let Some(filter) = &filter_opt {
                        let filter_lower = filter.to_lowercase();
                        stat.name.to_lowercase().contains(&filter_lower)
                    } else {
                        true
                    }
                }
            }),
            
            // Filter by user if specified
            Box::new({
                let user_opt = args.user.clone();
                // Create a closure that captures system info by value
                move |stat: &UsageInfo| -> bool {
                    if let Some(user) = &user_opt {
                        let user_lower = user.to_lowercase();
                        // Create a temporary System instance for this check
                        let mut temp_sys = System::new_with_specifics(
                            sysinfo::RefreshKind::new().with_processes(sysinfo::ProcessRefreshKind::new())
                        );
                        temp_sys.refresh_process(sysinfo::Pid::from_u32(stat.pid));
                        
                        if let Some(process) = temp_sys.process(sysinfo::Pid::from_u32(stat.pid)) {
                            if let Some(uid) = process.user_id() {
                                uid.to_string().contains(&user_lower)
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    } else {
                        true
                    }
                }
            }),
            
            // Hide kernel processes if requested
            Box::new({
                let no_kernel = args.no_kernel;
                move |stat: &UsageInfo| -> bool {
                    if no_kernel {
                        // Create a temporary System instance for this check
                        let mut temp_sys = System::new_with_specifics(
                            sysinfo::RefreshKind::new().with_processes(sysinfo::ProcessRefreshKind::new())
                        );
                        temp_sys.refresh_process(sysinfo::Pid::from_u32(stat.pid));
                        
                        if let Some(process) = temp_sys.process(sysinfo::Pid::from_u32(stat.pid)) {
                            !process.name().starts_with("kernel") && stat.pid >= 100
                        } else {
                            true
                        }
                    } else {
                        true
                    }
                }
            }),
            
            // Filter by CPU threshold if specified
            Box::new({
                let cpu_above = args.cpu_above;
                let cpu_below = args.cpu_below;
                move |stat: &UsageInfo| -> bool {
                    let above_check = if let Some(threshold) = cpu_above {
                        stat.cpu > threshold
                    } else {
                        true
                    };
                    
                    let below_check = if let Some(threshold) = cpu_below {
                        stat.cpu < threshold
                    } else {
                        true
                    };
                    
                    above_check && below_check
                }
            }),
            
            // Filter by memory threshold if specified
            Box::new({
                let mem_above = args.mem_above;
                let mem_below = args.mem_below;
                let human_readable = args.human_readable;
                move |stat: &UsageInfo| -> bool {
                    let above_check = if let Some(threshold) = mem_above {
                        if human_readable {
                            stat.mem > threshold
                        } else {
                            stat.mem > threshold * 1_000_000
                        }
                    } else {
                        true
                    };
                    
                    let below_check = if let Some(threshold) = mem_below {
                        if human_readable {
                            stat.mem < threshold
                        } else {
                            stat.mem < threshold * 1_000_000
                        }
                    } else {
                        true
                    };
                    
                    above_check && below_check
                }
            }),
        ];
        
        // Apply all filters
        stats.retain(|stat| filters.iter().all(|filter| filter(stat)));

        // Sort based on the specified criteria
        match args.sort_by {
            SortBy::Cpu => {
                stats.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap_or(Ordering::Less))
            }
            SortBy::Memory => stats.sort_by(|a, b| b.mem.cmp(&a.mem)),
            SortBy::Pid => stats.sort_by(|a, b| a.pid.cmp(&b.pid)),
            SortBy::StartTime => stats.sort_by(|a, b| a.start_time.cmp(&b.start_time)),
        }

        print(&mut stdout, stats, &args);

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
