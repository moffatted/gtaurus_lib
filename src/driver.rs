/*
 * @file driver.rs
 * @purpose Core FluidNC driver implementation managing communication threads and event distribution.
 * @author Ed Moffatt
 */
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use std::collections::VecDeque;
use std::net::TcpStream;
use serialport::SerialPort;
use std::io::Write;

use crate::types::{ActiveConnection, ConnectionStatus, DriverEventObserver, SerialWrapper};
use crate::traits::GCodeConnection;
use crate::transport::serial::{spawn_serial_reader, spawn_serial_writer};
use crate::transport::tcp::{spawn_tcp_reader, spawn_tcp_writer};

pub struct FluidNCDriver {
    pub conn: ActiveConnection,
    pub status: Arc<Mutex<ConnectionStatus>>,
    pub observer: Arc<dyn DriverEventObserver>,
    pub auto_connect_suspended: bool,
    pub subscribers: Arc<Mutex<Vec<std::sync::mpsc::Sender<String>>>>,
}

impl FluidNCDriver {
    pub fn new(observer: Arc<dyn DriverEventObserver>) -> Self {
        Self {
            conn: ActiveConnection::None,
            status: Arc::new(Mutex::new(ConnectionStatus::Disconnected)),
            observer,
            auto_connect_suspended: false,
            subscribers: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl GCodeConnection for FluidNCDriver {
    fn connect_serial(&mut self, port_name: &str, baud_rate: u32) -> Result<(), String> {
        self.disconnect();
        thread::sleep(Duration::from_millis(100));

        let port = serialport::new(port_name, baud_rate)
            .timeout(Duration::from_millis(100))
            .open()
            .map_err(|e| e.to_string())?;

        {
            let mut drain_port = port.try_clone().map_err(|e| e.to_string())?;
            let mut drain_buf = [0u8; 512];
            loop {
                match drain_port.read(&mut drain_buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        self.observer.emit(&format!("[GTaurus] Drained {} stale bytes from serial buffer", n));
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => break,
                    Err(_) => break,
                }
            }
        }

        thread::sleep(Duration::from_millis(150));

        let reader_clone = SerialWrapper(port.try_clone().map_err(|e| e.to_string())?);
        let writer_clone = SerialWrapper(port.try_clone().map_err(|e| e.to_string())?);
        let rt_inner: Box<dyn SerialPort + Send> = port.try_clone().map_err(|e| e.to_string())?;
        let rt_port = Arc::new(Mutex::new(rt_inner));

        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<String>();
        let pending_bytes = Arc::new(Mutex::new(0usize));
        let pending_lens = Arc::new(Mutex::new(VecDeque::<usize>::new()));
        let reset_signal = Arc::new(AtomicBool::new(false));

        if let Ok(mut s) = self.status.lock() {
            *s = ConnectionStatus::Serial(port_name.to_string());
        }

        self.observer.emit(&format!("[GTaurus] Connected via Serial: {}", port_name));

        spawn_serial_reader(reader_clone, pending_bytes.clone(), pending_lens.clone(), self.observer.clone(), self.status.clone());
        spawn_serial_writer(writer_clone, cmd_rx, pending_bytes.clone(), pending_lens.clone(), self.status.clone(), self.observer.clone(), reset_signal.clone());

        self.conn = ActiveConnection::Serial {
            _port: SerialWrapper(port),
            rt_port,
            cmd_tx,
            pending_bytes,
            pending_lens,
            reset_signal,
        };
        Ok(())
    }

    fn connect_telnet(&mut self, host: &str, port: u16) -> Result<(), String> {
        self.disconnect();
        let addr = format!("{}:{}", host, port);
        let stream = TcpStream::connect(&addr).map_err(|e| format!("Cannot connect to {addr}: {e}"))?;
        stream.set_read_timeout(Some(Duration::from_millis(100))).map_err(|e| e.to_string())?;

        let reader_clone = stream.try_clone().map_err(|e| e.to_string())?;
        let writer_arc = Arc::new(Mutex::new(stream.try_clone().map_err(|e| e.to_string())?));
        let rt_stream = writer_arc.clone();

        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<String>();
        let reset_signal = Arc::new(AtomicBool::new(false));

        if let Ok(mut s) = self.status.lock() {
            *s = ConnectionStatus::Telnet(addr.clone());
        }

        self.observer.emit(&format!("[GTaurus] Connected via Telnet: {addr}"));

        spawn_tcp_reader(reader_clone, self.observer.clone(), self.status.clone());
        spawn_tcp_writer(writer_arc, cmd_rx, self.status.clone(), self.observer.clone(), reset_signal.clone());

        self.conn = ActiveConnection::Telnet {
            _stream: stream,
            rt_stream,
            cmd_tx,
            reset_signal,
        };
        Ok(())
    }

    fn send_command(&mut self, cmd: String) -> Result<(), String> {
        if self.get_status() == "Disconnected" {
            return Err("Not connected".to_string());
        }
        match &self.conn {
            ActiveConnection::Serial { cmd_tx, .. } => cmd_tx.send(cmd).map_err(|e| e.to_string()),
            ActiveConnection::Telnet { cmd_tx, .. } => cmd_tx.send(cmd).map_err(|e| e.to_string()),
            ActiveConnection::None => Err("Not connected".to_string()),
        }
    }

    fn send_realtime(&mut self, byte: u8) -> Result<(), String> {
        if self.get_status() == "Disconnected" {
            return Err("Not connected".to_string());
        }
        match &self.conn {
            ActiveConnection::Serial { rt_port, pending_bytes, pending_lens, reset_signal, .. } => {
                {
                    let mut p = rt_port.lock().map_err(|_| "Poisoned".to_string())?;
                    p.write_all(&[byte]).map_err(|e| e.to_string())?;
                    p.flush().map_err(|e| e.to_string())?;
                }
                if byte == 0x18 {
                    *pending_bytes.lock().unwrap() = 0;
                    pending_lens.lock().unwrap().clear();
                    reset_signal.store(true, Ordering::Relaxed);
                    self.observer.emit("[GTaurus] Soft Reset (0x18) - Local buffer cleared, writer drain signaled");
                }
                Ok(())
            }
            ActiveConnection::Telnet { rt_stream, reset_signal, .. } => {
                let mut s = rt_stream.lock().map_err(|_| "Poisoned".to_string())?;
                s.write_all(&[byte]).map_err(|e| e.to_string())?;
                s.flush().map_err(|e| e.to_string())?;
                if byte == 0x18 {
                    reset_signal.store(true, Ordering::Relaxed);
                    self.observer.emit("[GTaurus] Soft Reset (0x18) - Writer drain signaled");
                }
                Ok(())
            }
            ActiveConnection::None => Err("Not connected".to_string()),
        }
    }

    fn disconnect(&mut self) {
        self.conn = ActiveConnection::None;
        if let Ok(mut s) = self.status.lock() {
            *s = ConnectionStatus::Disconnected;
        }
    }

    fn get_status(&self) -> String {
        if let Ok(s) = self.status.lock() {
            s.to_string()
        } else {
            "Disconnected".to_string()
        }
    }

    fn add_rx_subscriber(&mut self, tx: std::sync::mpsc::Sender<String>) {
        if let Ok(mut subs) = self.subscribers.lock() {
            subs.push(tx);
        }
    }

    fn set_auto_connect_suspended(&mut self, suspended: bool) {
        self.auto_connect_suspended = suspended;
    }

    fn is_auto_connect_suspended(&self) -> bool {
        self.auto_connect_suspended
    }
}
