use std::sync::{Arc, Mutex};
use std::collections::VecDeque;
use std::net::TcpStream;
use std::sync::atomic::AtomicBool;
use serialport::SerialPort;

pub const MAX_BUFFER_SIZE: usize = 127;
pub const RX_EVENT: &str = "rx_event";

pub struct SerialWrapper(pub Box<dyn SerialPort>);
unsafe impl Send for SerialWrapper {}

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

pub trait DriverEventObserver: Send + Sync {
    fn emit(&self, line: &str);
}

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
