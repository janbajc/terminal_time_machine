use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::time::{Duration, Instant};
use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use serde_json::Value;
use clap::{Arg, Command};

#[derive(Debug)]
struct TerminalEvent {
    time_ms: u64,
    event_type: String,
    data: Vec<u8>,
}

fn load_session(filename: &str) -> Result<Vec<TerminalEvent>> {
    let file = File::open(filename)?;
    let reader = BufReader::new(file);
    let mut events = Vec::new();

    for line in reader.lines() {
        let line = line?;
        let json: Value = serde_json::from_str(&line)?;
        
        let time_ms = json["time_ms"].as_u64().unwrap_or(0);
        let event_type = json["type"].as_str().unwrap_or("").to_string();
        let data_b64 = json["data"].as_str().unwrap_or("");
        let data = general_purpose::STANDARD.decode(data_b64)?;
        
        events.push(TerminalEvent {
            time_ms,
            event_type,
            data,
        });
    }

    Ok(events)
}

fn play_session(events: Vec<TerminalEvent>, speed: f64, show_input: bool) -> Result<()> {
    if events.is_empty() {
        println!("No events to play back.");
        return Ok(());
    }

    println!("🎬 Starting Terminal Time Machine playback...");
    println!("📊 Total events: {}", events.len());
    println!("⚡ Speed: {}x", speed);
    println!("📝 Show input: {}", if show_input { "Yes" } else { "No" });
    println!("⏱️  Duration: {:.2}s", events.last().unwrap().time_ms as f64 / 1000.0);
    println!("───────────────────────────────────────");
    
    // Wait a moment before starting
    std::thread::sleep(Duration::from_secs(2));
    
    let start_time = Instant::now();
    let first_event_time = events[0].time_ms;
    
    for event in events {
        // Calculate when this event should happen
        let event_offset_ms = event.time_ms - first_event_time;
        let target_time = Duration::from_millis((event_offset_ms as f64 / speed) as u64);
        
        // Wait until it's time for this event
        let elapsed = start_time.elapsed();
        if target_time > elapsed {
            std::thread::sleep(target_time - elapsed);
        }
        
        // Only show output events, or show input if requested
        if event.event_type == "output" || (show_input && event.event_type == "input") {
            std::io::stdout().write_all(&event.data)?;
            std::io::stdout().flush()?;
        }
    }
    
    println!("\n───────────────────────────────────────");
    println!("🎬 Playback complete!");
    
    Ok(())
}

fn show_session_info(events: &[TerminalEvent]) -> Result<()> {
    if events.is_empty() {
        println!("No events found.");
        return Ok(());
    }
    
    let duration_ms = events.last().unwrap().time_ms - events[0].time_ms;
    let input_events = events.iter().filter(|e| e.event_type == "input").count();
    let output_events = events.iter().filter(|e| e.event_type == "output").count();
    
    println!("📊 Session Information:");
    println!("├─ Duration: {:.2} seconds", duration_ms as f64 / 1000.0);
    println!("├─ Total events: {}", events.len());
    println!("├─ Input events: {}", input_events);
    println!("├─ Output events: {}", output_events);
    println!("└─ First event: {} ms", events[0].time_ms);
    
    Ok(())
}

fn main() -> Result<()> {
    let matches = Command::new("Terminal Time Machine Player")
        .version("1.0")
        .about("Replay terminal sessions like a movie!")
        .arg(
            Arg::new("file")
                .short('f')
                .long("file")
                .value_name("FILE")
                .help("JSONL recording file to play")
                .default_value("session_record.jsonl")
        )
        .arg(
            Arg::new("speed")
                .short('s')
                .long("speed")
                .value_name("MULTIPLIER")
                .help("Playback speed multiplier (e.g., 2.0 for 2x speed)")
                .default_value("1.0")
        )
        .arg(
            Arg::new("info")
                .short('i')
                .long("info")
                .help("Show session information only")
                .action(clap::ArgAction::SetTrue)
        )
        .arg(
            Arg::new("show-input")
                .long("show-input")
                .help("Show input events during playback")
                .action(clap::ArgAction::SetTrue)
        )
        .get_matches();

    let filename = matches.get_one::<String>("file").unwrap();
    let speed: f64 = matches.get_one::<String>("speed").unwrap().parse()?;
    let info_only = matches.get_flag("info");
    let show_input = matches.get_flag("show-input");

    println!("🎞️  Terminal Time Machine Player");
    println!("Loading session: {}", filename);
    
    let events = load_session(filename)?;
    
    if info_only {
        show_session_info(&events)?;
    } else {
        show_session_info(&events)?;
        println!("\nPress Enter to start playback...");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        
        play_session(events, speed, show_input)?;
    }
    
    Ok(())
}