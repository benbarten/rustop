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
use std::{
    io::{Stdout, Write, stdout},
    sync::atomic::AtomicBool,
};
use sysinfo::System;

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

fn print(stdout: &mut Stdout, stats: Vec<UsageInfo>) {
    let (_, rows) = size().unwrap_or((0, 0));
    let lines_to_print = (rows as usize).saturating_sub(2); // Reserve 2 lines for header

    execute!(
        stdout,
        cursor::MoveTo(0, 0),
        Clear(ClearType::All),
        SetForegroundColor(Color::Green),
        Print("\r\n"),
        Print(format!(
            "{:<6} {:<25} {:>10} {:>12}\n",
            "PID", "COMMAND", "CPU (%)", "Memory (MB)"
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
                stat.mem / 1_000_000, // Convert memory to MB
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
    let term = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&term))?;

    let mut sys = System::new_all();
    let num_cpus = sys.cpus().len() as f64;

    let mut stdout = stdout();
    let _ = setup_terminal(&mut stdout);

    while !term.load(atomic::Ordering::Relaxed) {
        let sample = sample();
        thread::sleep(Duration::from_secs(1));
        let mut stats = stats(num_cpus, sample);

        stats.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap_or(Ordering::Less));

        print(&mut stdout, stats);
        sys.refresh_all();
    }

    let _ = cleanup_terminal(&mut stdout);

    Ok(())
}
