//! Shared types and constants for transport and driver layers.

use std::sync::{Arc, Mutex};
use std::collections::VecDeque;
use std::net::TcpStream;
use std::sync::atomic::AtomicBool;
use serialport::SerialPort;

/// Maximum sender-side character budget used for serial flow control.
pub const MAX_BUFFER_SIZE: usize = 127;
/// Event name used by consumers for RX line notifications.
pub const RX_EVENT: &str = "rx_event";

/// Wrapper for serial port trait objects used in connection state.
pub struct SerialWrapper(pub Box<dyn SerialPort>);
/// # Safety
/// `SerialWrapper` is shared only behind synchronization primitives.
unsafe impl Send for SerialWrapper {}

#[derive(Clone, Debug, PartialEq)]
/// Connection state for the active machine link.
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

/// Observer callback for driver-emitted text lines.
pub trait DriverEventObserver: Send + Sync {
    /// Emit an informational or machine output line.
    fn emit(&self, line: &str);
}

/// Internal active transport state and associated channels/resources.
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
