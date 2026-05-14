use std::path::Path;

use super::recorder::StatsRecorder;

pub async fn print_stats(project_root: &Path) {
    let recorder = StatsRecorder::new(project_root);

    if !recorder.db_exists() {
        println!("No stats data found.");
        return;
    }

    println!("Usage Stats (last 7 days)\n");

    print_tool_usage(&recorder).await;
    println!();
    print_top_files(&recorder, project_root).await;
    println!();
    print_server_usage(&recorder).await;
}

async fn print_tool_usage(recorder: &StatsRecorder) {
    println!("Tool Usage:");
    match recorder.query_tool_usage().await {
        Ok(tools) if tools.is_empty() => println!("  (no data)"),
        Ok(tools) => {
            for t in &tools {
                println!(
                    "  {:<22} {:>3} calls  {:>5}ms avg  {:>2} errors",
                    t.tool, t.calls, t.avg_ms, t.errors
                );
            }
        }
        Err(e) => eprintln!("  error querying tool usage: {e}"),
    }
}

async fn print_top_files(recorder: &StatsRecorder, project_root: &Path) {
    println!("Top Files:");
    match recorder.query_top_files().await {
        Ok(files) if files.is_empty() => println!("  (no data)"),
        Ok(files) => {
            for f in &files {
                let display_path = Path::new(&f.file_path)
                    .strip_prefix(project_root)
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| f.file_path.clone());
                println!("  {:<40} {:>3} calls", display_path, f.calls);
            }
        }
        Err(e) => eprintln!("  error querying files: {e}"),
    }
}

async fn print_server_usage(recorder: &StatsRecorder) {
    println!("Language Servers:");
    match recorder.query_server_usage().await {
        Ok(servers) if servers.is_empty() => println!("  (no data)"),
        Ok(servers) => {
            for s in &servers {
                let startup = format_duration_ms(s.avg_startup_ms);
                let uptime = format_duration_secs(s.total_uptime_secs);
                println!(
                    "  {:<10} started {:>2}\u{00d7}  avg startup {:>6}  uptime {} total",
                    s.language, s.startups, startup, uptime
                );
            }
        }
        Err(e) => eprintln!("  error querying servers: {e}"),
    }
}

fn format_duration_ms(ms: i64) -> String {
    if ms < 1000 {
        format!("{ms}ms")
    } else {
        format!("{:.1}s", ms as f64 / 1000.0)
    }
}

fn format_duration_secs(secs: i64) -> String {
    if secs < 0 {
        return "0s".to_string();
    }
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        let m = secs / 60;
        let s = secs % 60;
        format!("{m}m {s}s")
    } else {
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        format!("{h}h {m}m")
    }
}
