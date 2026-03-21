//! Serial transport workers with character-counting flow control.
use std::io::{BufRead, BufReader, Write};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use std::collections::VecDeque;
use crate::types::{SerialWrapper, ConnectionStatus, DriverEventObserver, MAX_BUFFER_SIZE};

/// Spawn the serial reader thread.
///
/// Emits received lines to the observer and releases pending buffer budget on
/// `ok`/`error:` responses.
///
/// # Errors
///
/// This function does not return errors directly; read errors are handled inside
/// the worker by transitioning connection state to `Disconnected`.
///
/// # Panics
///
/// May panic if any internal mutex is poisoned when calling `.unwrap()`.
pub fn spawn_serial_reader(
    reader_port: SerialWrapper,
    pending_bytes: Arc<Mutex<usize>>,
    pending_lens: Arc<Mutex<VecDeque<usize>>>,
    observer: Arc<dyn DriverEventObserver>,
    status: Arc<Mutex<ConnectionStatus>>,
) {
    thread::spawn(move || {
        let mut reader = BufReader::new(reader_port.0);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    observer.emit("[GTaurus] Serial EOF");
                    if let Ok(mut s) = status.lock() {
                        *s = ConnectionStatus::Disconnected;
                    }
                    break;
                }
                Ok(_) => {
                    let trimmed = line.trim().to_string();
                    if !trimmed.is_empty() {
                        let is_ok = trimmed == "ok";
                        let is_error = trimmed.starts_with("error:");

                        if is_ok || is_error {
                            let mut lenses = pending_lens.lock().unwrap();
                            if let Some(len) = lenses.pop_front() {
                                let mut bytes = pending_bytes.lock().unwrap();
                                *bytes = bytes.saturating_sub(len);
                                let remaining = *bytes;
                                observer.emit(&format!(
                                    "[GTaurus] Buffer Release: {} bytes (remaining: {})",
                                    len, remaining
                                ));
                            }
                        }
                        observer.emit(&trimmed);
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                    if let Ok(s) = status.lock() {
                        if matches!(*s, ConnectionStatus::Disconnected) {
                            break;
                        }
                    }
                    continue;
                }
                Err(_) => {
                    observer.emit("[GTaurus] Serial read error");
                    if let Ok(mut s) = status.lock() {
                        *s = ConnectionStatus::Disconnected;
                    }
                    break;
                }
            }
        }
    });
}

/// Spawn the serial writer thread.
///
/// Pulls queued commands, waits for available buffer budget, and writes commands
/// with trailing newline.
///
/// # Errors
///
/// This function does not return errors directly; write/channel failures are
/// handled inside the worker loop.
///
/// # Panics
///
/// May panic if any internal mutex is poisoned when calling `.unwrap()`.
pub fn spawn_serial_writer(
    mut writer_port: SerialWrapper,
    rx: std::sync::mpsc::Receiver<String>,
    pending_bytes: Arc<Mutex<usize>>,
    pending_lens: Arc<Mutex<VecDeque<usize>>>,
    status: Arc<Mutex<ConnectionStatus>>,
    observer: Arc<dyn DriverEventObserver>,
    reset_signal: Arc<AtomicBool>,
) {
    thread::spawn(move || {
        loop {
            let cmd = match rx.recv() {
                Ok(cmd) => cmd,
                Err(_) => break, // channel closed
            };

            if reset_signal.load(Ordering::Relaxed) {
                let mut drained = 1;
                while rx.try_recv().is_ok() {
                    drained += 1;
                }
                reset_signal.store(false, Ordering::Relaxed);
                observer.emit(&format!(
                    "[GTaurus] Writer: Drained {} queued command(s) after soft reset",
                    drained
                ));
                thread::sleep(Duration::from_millis(500));
                continue;
            }

            let cmd_len = cmd.len() + 1;
            loop {
                if let Ok(s) = status.lock() {
                    if matches!(*s, ConnectionStatus::Disconnected) {
                        return;
                    }
                }
                if reset_signal.load(Ordering::Relaxed) {
                    break;
                }
                if *pending_bytes.lock().unwrap() + cmd_len < MAX_BUFFER_SIZE {
                    break;
                }
                thread::sleep(Duration::from_millis(1));
            }

            if reset_signal.load(Ordering::Relaxed) {
                continue;
            }

            let full_cmd = format!("{}\n", cmd);
            {
                let mut bytes = pending_bytes.lock().unwrap();
                *bytes += cmd_len;
                pending_lens.lock().unwrap().push_back(cmd_len);
                let current = *bytes;
                observer.emit(&format!(
                    "[GTaurus] TX: {} (pending: {} bytes)",
                    cmd, current
                ));
            }
            if writer_port.0.write_all(full_cmd.as_bytes()).is_err()
                || writer_port.0.flush().is_err()
            {
                observer.emit("[GTaurus] Serial write error");
                if let Ok(mut s) = status.lock() {
                    *s = ConnectionStatus::Disconnected;
                }
                break;
            }
        }
    });
}
