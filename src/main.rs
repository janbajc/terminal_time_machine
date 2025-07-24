use portable_pty::{CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::time::Instant;
use anyhow::Result;
use std::fs::File;
use std::io::BufWriter;
use base64::{engine::general_purpose, Engine as _};
use nix::sys::termios::{self, LocalFlags, InputFlags, OutputFlags, SetArg};
use std::os::fd::BorrowedFd;

fn set_raw_mode() -> anyhow::Result<termios::Termios> {
    let stdin_fd = unsafe { BorrowedFd::borrow_raw(0) }; // stdin is fd 0
    let original_termios = termios::tcgetattr(&stdin_fd)?;
    
    let mut raw_termios = original_termios.clone();
    
    // Disable echo and canonical mode
    raw_termios.local_flags.remove(LocalFlags::ECHO | LocalFlags::ICANON);
    raw_termios.input_flags.remove(InputFlags::ICRNL | InputFlags::IXON);
    raw_termios.output_flags.remove(OutputFlags::OPOST);
    
    termios::tcsetattr(&stdin_fd, SetArg::TCSANOW, &raw_termios)?;
    
    Ok(original_termios)
}

fn restore_terminal(original_termios: &termios::Termios) -> anyhow::Result<()> {
    let stdin_fd = unsafe { BorrowedFd::borrow_raw(0) };
    termios::tcsetattr(&stdin_fd, SetArg::TCSANOW, original_termios)?;
    Ok(())
}

fn main() -> Result<()> {
    // Set terminal to raw mode and save original settings
    let original_termios = set_raw_mode()?;
    let pty_system = portable_pty::native_pty_system();
    let size = PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    };
    let pair = pty_system.openpty(size)?;

    let cmd = CommandBuilder::new("bash");
    let mut child = pair.slave.spawn_command(cmd)?;

    let master = pair.master;

    // Prepare to read from PTY and get writer
    let mut reader = master.try_clone_reader()?;
    let mut writer_for_input = master.take_writer()?;

    // Prepare file for recording
    let file = File::create("session_record.jsonl")?;
    let mut writer = BufWriter::new(file);

    let start = Instant::now();

    // Create a writer for sending input to the PTY
    let mut input_recorder = writer.get_ref().try_clone()?;

    // Spawn input forwarding + recording thread
    std::thread::spawn(move || {
        let mut stdin = std::io::stdin();
        let mut buf = [0u8; 1024];
        loop {
            match stdin.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let elapsed = start.elapsed().as_millis();

                    if writer_for_input.write_all(&buf[..n]).is_err() {
                        break;
                    }
                    let _ = writer_for_input.flush();

                    let record = serde_json::json!({
                        "time_ms": elapsed,
                        "type": "input",
                        "data": general_purpose::STANDARD.encode(&buf[..n]),
                    });
                    let _ = writeln!(input_recorder, "{}", record.to_string());
                    let _ = input_recorder.flush();
                }
                Err(_) => break,
            }
        }
    });

    // Main thread: output reader
    let mut output_buf = [0u8; 4096];
    loop {
        match reader.read(&mut output_buf) {
            Ok(0) => break,
            Ok(n) => {
                let elapsed = start.elapsed().as_millis();

                std::io::stdout().write_all(&output_buf[..n])?;
                std::io::stdout().flush()?;

                let record = serde_json::json!({
                    "time_ms": elapsed,
                    "type": "output",
                    "data": general_purpose::STANDARD.encode(&output_buf[..n]),
                });
                writeln!(writer, "{}", record.to_string())?;
                writer.flush()?;
            }
            Err(e) => {
                eprintln!("Error reading from PTY: {:?}", e);
                break;
            }
        }
    }

    child.wait()?;
    
    // Restore terminal settings before exiting
    restore_terminal(&original_termios)?;
    
    Ok(())
}