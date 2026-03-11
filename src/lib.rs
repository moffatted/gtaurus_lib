/*
 * @file lib.rs
 * @purpose Core communication library for FluidNC, providing Serial and Telnet transport implementations with local buffering and real-time command support.
 */
use serialport::SerialPort;
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub const MAX_BUFFER_SIZE: usize = 127;

/// common event name if needed
pub const RX_EVENT: &str = "fluidnc://rx";

// ─── SerialWrapper (Send-safe box) ───────────────────────────────────────────
pub struct SerialWrapper(pub Box<dyn SerialPort>);
unsafe impl Send for SerialWrapper {}

// ─── Connection status ────────────────────────────────────────────────────────
#[derive(Clone, Debug, PartialEq)]
pub enum ConnectionStatus {
    Disconnected,
    Serial(String), // port name
    Telnet(String), // host:port
}

impl std::fmt::Display for ConnectionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionStatus::Disconnected => write!(f, "Disconnected"),
            ConnectionStatus::Serial(p) => write!(f, "Serial: {p}"),
            ConnectionStatus::Telnet(h) => write!(f, "WiFi: {h}"),
        }
    }
}

// ─── Observer Trait ──────────────────────────────────────────────────────────
pub trait DriverEventObserver: Send + Sync {
    fn emit(&self, line: &str);
}

// ─── Public trait ─────────────────────────────────────────────────────────────
pub trait GCodeConnection: Send {
    fn connect_serial(&mut self, port_name: &str, baud_rate: u32) -> Result<(), String>;
    fn connect_telnet(&mut self, host: &str, port: u16) -> Result<(), String>;
    fn send_command(&mut self, cmd: String) -> Result<(), String>;
    fn send_realtime(&mut self, byte: u8) -> Result<(), String>;
    fn disconnect(&mut self);
    fn get_status(&self) -> String;
    fn add_rx_subscriber(&mut self, tx: std::sync::mpsc::Sender<String>);
    fn set_auto_connect_suspended(&mut self, suspended: bool);
    fn is_auto_connect_suspended(&self) -> bool;
}

// ─── Active connection container ──────────────────────────────────────────────
pub enum ActiveConnection {
    None,
    Serial {
        _port: SerialWrapper,
        rt_port: Arc<Mutex<Box<dyn SerialPort + Send>>>,
        cmd_tx: std::sync::mpsc::Sender<String>,
        pending_bytes: Arc<Mutex<usize>>,
        pending_lens: Arc<Mutex<VecDeque<usize>>>,
        reset_signal: Arc<AtomicBool>,
    },
    Telnet {
        _stream: TcpStream,
        rt_stream: Arc<Mutex<TcpStream>>,
        cmd_tx: std::sync::mpsc::Sender<String>,
        reset_signal: Arc<AtomicBool>,
    },
}

// ─── FluidNCDriver ────────────────────────────────────────────────────────────
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
                // Block waiting for the next command
                let cmd = match rx.recv() {
                    Ok(cmd) => cmd,
                    Err(_) => break, // channel closed
                };

                // Check if a soft reset was signaled — drain all queued commands
                if reset_signal.load(Ordering::Relaxed) {
                    let mut drained = 1; // count the current cmd
                    while rx.try_recv().is_ok() {
                        drained += 1;
                    }
                    reset_signal.store(false, Ordering::Relaxed);
                    observer.emit(&format!(
                        "[GTaurus] Writer: Drained {} queued command(s) after soft reset",
                        drained
                    ));
                    // Brief pause for FluidNC to finish rebooting
                    thread::sleep(Duration::from_millis(500));
                    continue;
                }

                let cmd_len = cmd.len() + 1;
                loop {
                    if let Ok(s) = status.lock() {
                        if matches!(*s, ConnectionStatus::Disconnected) {
                            return; // exit thread
                        }
                    }
                    // Also break out of buffer-wait if reset was signaled
                    if reset_signal.load(Ordering::Relaxed) {
                        break;
                    }
                    if *pending_bytes.lock().unwrap() + cmd_len < MAX_BUFFER_SIZE {
                        break;
                    }
                    thread::sleep(Duration::from_millis(1));
                }

                // Final check before writing — if reset was signaled while waiting,
                // loop back and let the drain logic at the top handle it
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
        thread::spawn(move || {
            loop {
                let cmd = match rx.recv() {
                    Ok(cmd) => cmd,
                    Err(_) => break,
                };

                // Check if a soft reset was signaled — drain all queued commands
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
                    if let Ok(mut s) = status.lock() {
                        *s = ConnectionStatus::Disconnected;
                    }
                    break;
                }
            }
        });
    }
}

impl GCodeConnection for FluidNCDriver {
    fn connect_serial(&mut self, port_name: &str, baud_rate: u32) -> Result<(), String> {
        self.disconnect();

        // Brief pause to let the OS fully release the port from any prior connection
        thread::sleep(Duration::from_millis(100));

        let port = serialport::new(port_name, baud_rate)
            .timeout(Duration::from_millis(100))
            .open()
            .map_err(|e| e.to_string())?;

        // Drain any stale data sitting in the OS serial buffer from a previous
        // session or from the controller's boot output after a power cycle.
        {
            let mut drain_port = port.try_clone().map_err(|e| e.to_string())?;
            let mut drain_buf = [0u8; 512];
            loop {
                match drain_port.read(&mut drain_buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        self.observer.emit(&format!(
                            "[GTaurus] Drained {} stale bytes from serial buffer",
                            n
                        ));
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => break,
                    Err(_) => break,
                }
            }
        }

        // Let the controller stabilize after the port is opened
        thread::sleep(Duration::from_millis(150));

        let reader_clone = SerialWrapper(port.try_clone().map_err(|e| e.to_string())?);
        let writer_clone = SerialWrapper(port.try_clone().map_err(|e| e.to_string())?);
        let rt_inner: Box<dyn SerialPort + Send> = port.try_clone().map_err(|e| e.to_string())?;
        let rt_port = Arc::new(Mutex::new(rt_inner));

        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<String>();
        let pending_bytes = Arc::new(Mutex::new(0usize));
        let pending_lens = Arc::new(Mutex::new(VecDeque::<usize>::new()));
        let reset_signal = Arc::new(AtomicBool::new(false));

        // IMPORTANT: Set status BEFORE spawning threads. Both the reader and
        // writer threads check the status on timeout/startup and will exit
        // immediately if they see Disconnected — a race condition that caused
        // reconnection failures after power-cycling the controller.
        if let Ok(mut s) = self.status.lock() {
            *s = ConnectionStatus::Serial(port_name.to_string());
        }

        self.observer
            .emit(&format!("[GTaurus] Connected via Serial: {}", port_name));

        Self::spawn_serial_reader(
            reader_clone,
            pending_bytes.clone(),
            pending_lens.clone(),
            self.observer.clone(),
            self.status.clone(),
        );
        Self::spawn_serial_writer(
            writer_clone,
            cmd_rx,
            pending_bytes.clone(),
            pending_lens.clone(),
            self.status.clone(),
            self.observer.clone(),
            reset_signal.clone(),
        );

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
        let stream =
            TcpStream::connect(&addr).map_err(|e| format!("Cannot connect to {addr}: {e}"))?;
        stream
            .set_read_timeout(Some(Duration::from_millis(100)))
            .map_err(|e| e.to_string())?;

        let reader_clone = stream.try_clone().map_err(|e| e.to_string())?;
        let writer_arc = Arc::new(Mutex::new(stream.try_clone().map_err(|e| e.to_string())?));
        let rt_stream = writer_arc.clone();

        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<String>();
        let reset_signal = Arc::new(AtomicBool::new(false));

        // Set status BEFORE spawning threads to prevent the same race
        // condition as in connect_serial (threads exit if they see Disconnected).
        if let Ok(mut s) = self.status.lock() {
            *s = ConnectionStatus::Telnet(addr.clone());
        }

        self.observer
            .emit(&format!("[GTaurus] Connected via Telnet: {addr}"));

        Self::spawn_tcp_reader(reader_clone, self.observer.clone(), self.status.clone());
        Self::spawn_tcp_writer(
            writer_arc,
            cmd_rx,
            self.status.clone(),
            self.observer.clone(),
            reset_signal.clone(),
        );

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
            ActiveConnection::Serial {
                rt_port,
                pending_bytes,
                pending_lens,
                reset_signal,
                ..
            } => {
                {
                    let mut p = rt_port.lock().map_err(|_| "Poisoned".to_string())?;
                    p.write_all(&[byte]).map_err(|e| e.to_string())?;
                    p.flush().map_err(|e| e.to_string())?;
                }

                // If soft reset (Ctrl-X / 0x18), clear pending buffer state and
                // signal the writer thread to drain its command queue
                if byte == 0x18 {
                    *pending_bytes.lock().unwrap() = 0;
                    pending_lens.lock().unwrap().clear();
                    reset_signal.store(true, Ordering::Relaxed);
                    self.observer.emit(
                        "[GTaurus] Soft Reset (0x18) - Local buffer cleared, writer drain signaled",
                    );
                }
                Ok(())
            }
            ActiveConnection::Telnet {
                rt_stream,
                reset_signal,
                ..
            } => {
                let mut s = rt_stream.lock().map_err(|_| "Poisoned".to_string())?;
                s.write_all(&[byte]).map_err(|e| e.to_string())?;
                s.flush().map_err(|e| e.to_string())?;

                // Signal writer thread to drain on soft reset
                if byte == 0x18 {
                    reset_signal.store(true, Ordering::Relaxed);
                    self.observer
                        .emit("[GTaurus] Soft Reset (0x18) - Writer drain signaled");
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    struct MockObserver {
        messages: Mutex<Vec<String>>,
    }

    impl MockObserver {
        fn new() -> Self {
            Self {
                messages: Mutex::new(Vec::new()),
            }
        }
    }

    impl DriverEventObserver for MockObserver {
        fn emit(&self, line: &str) {
            self.messages.lock().unwrap().push(line.to_string());
        }
    }

    #[test]
    fn test_connection_status_display() {
        assert_eq!(ConnectionStatus::Disconnected.to_string(), "Disconnected");
        assert_eq!(
            ConnectionStatus::Serial("COM3".to_string()).to_string(),
            "Serial: COM3"
        );
        assert_eq!(
            ConnectionStatus::Telnet("192.168.1.100:23".to_string()).to_string(),
            "WiFi: 192.168.1.100:23"
        );
    }

    #[test]
    fn test_fluidnc_driver_initialization() {
        let observer = Arc::new(MockObserver::new());
        let driver = FluidNCDriver::new(observer);

        assert_eq!(driver.get_status(), "Disconnected");
        assert!(!driver.is_auto_connect_suspended());
        let status = driver.status.lock().unwrap();
        assert_eq!(*status, ConnectionStatus::Disconnected);
    }

    #[test]
    fn test_fluidnc_driver_suspend_auto_connect() {
        let observer = Arc::new(MockObserver::new());
        let mut driver = FluidNCDriver::new(observer);

        driver.set_auto_connect_suspended(true);
        assert!(driver.is_auto_connect_suspended());

        driver.set_auto_connect_suspended(false);
        assert!(!driver.is_auto_connect_suspended());
    }

    #[test]
    fn test_send_command_disconnected() {
        let observer = Arc::new(MockObserver::new());
        let mut driver = FluidNCDriver::new(observer);

        let result = driver.send_command("G0 X10".to_string());
        assert_eq!(result.unwrap_err(), "Not connected");
    }

    #[test]
    fn test_send_realtime_disconnected() {
        let observer = Arc::new(MockObserver::new());
        let mut driver = FluidNCDriver::new(observer);

        let result = driver.send_realtime(0x85); // Jog Cancel
        assert_eq!(result.unwrap_err(), "Not connected");
    }

    #[test]
    fn test_add_rx_subscriber() {
        let observer = Arc::new(MockObserver::new());
        let mut driver = FluidNCDriver::new(observer);

        let (tx, _rx) = mpsc::channel();
        driver.add_rx_subscriber(tx);

        let subs = driver.subscribers.lock().unwrap();
        assert_eq!(subs.len(), 1);
    }
}
