/*
 * @file tcp.rs
 * @purpose Back-end threads for Telnet (TCP) machine communication.
 * @author Ed Moffatt
 */
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use crate::types::{ConnectionStatus, DriverEventObserver};

pub fn spawn_tcp_reader(
    stream: TcpStream,
    observer: Arc<dyn DriverEventObserver>,
    status: Arc<Mutex<ConnectionStatus>>,
) {
    thread::spawn(move || {
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    observer.emit("[GTaurus] Telnet connection closed");
                    if let Ok(mut s) = status.lock() {
                        *s = ConnectionStatus::Disconnected;
                    }
                    break;
                }
                Ok(_) => {
                    let trimmed = line.trim().to_string();
                    if !trimmed.is_empty() {
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
                Err(e) => {
                    observer.emit(&format!("[GTaurus] Telnet read error: {e}"));
                    if let Ok(mut s) = status.lock() {
                        *s = ConnectionStatus::Disconnected;
                    }
                    break;
                }
            }
        }
    });
}

pub fn spawn_tcp_writer(
    stream: Arc<Mutex<TcpStream>>,
    rx: std::sync::mpsc::Receiver<String>,
    status: Arc<Mutex<ConnectionStatus>>,
    observer: Arc<dyn DriverEventObserver>,
    reset_signal: Arc<AtomicBool>,
) {
    let _status = status; // Keep for possible future use
    thread::spawn(move || {
        loop {
            let cmd = match rx.recv() {
                Ok(cmd) => cmd,
                Err(_) => break,
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

            let mut s = stream.lock().unwrap();
            let full_cmd = format!("{}\n", cmd);
            if s.write_all(full_cmd.as_bytes()).is_err() || s.flush().is_err() {
                observer.emit("[GTaurus] Telnet write error");
                // Note: We'd need the status handle here to disconnect, 
                // but usually the reader will catch the error and disconnect.
                break;
            }
        }
    });
}
