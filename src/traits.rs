//! Trait definitions for machine communication backends.

/// Core machine communication API implemented by transport drivers.
///
/// # Example
///
/// ```no_run
/// use std::sync::Arc;
/// use gtaurus_common::{DriverEventObserver, FluidNCDriver, GCodeConnection};
///
/// struct Observer;
/// impl DriverEventObserver for Observer {
///     fn emit(&self, line: &str) {
///         println!("{}", line);
///     }
/// }
///
/// let mut driver = FluidNCDriver::new(Arc::new(Observer));
/// driver.connect_serial("/dev/ttyUSB0", 115200)?;
/// driver.send_command("$I".to_string())?;
/// driver.send_realtime(0x3F)?; // Status report query
/// driver.disconnect();
/// # Ok::<(), String>(())
/// ```
pub trait GCodeConnection: Send {
    /// Connect using a serial device path and baud rate.
    fn connect_serial(&mut self, port_name: &str, baud_rate: u32) -> Result<(), String>;
    /// Connect using Telnet (TCP) host and port.
    fn connect_telnet(&mut self, host: &str, port: u16) -> Result<(), String>;
    /// Queue a G-code command for transmission.
    fn send_command(&mut self, cmd: String) -> Result<(), String>;
    /// Send a realtime control byte immediately.
    fn send_realtime(&mut self, byte: u8) -> Result<(), String>;
    /// Disconnect and release active transport resources.
    fn disconnect(&mut self);
    /// Return a human-readable connection status string.
    fn get_status(&self) -> String;
    /// Register a subscriber channel for inbound machine output.
    fn add_rx_subscriber(&mut self, tx: std::sync::mpsc::Sender<String>);
    /// Enable or disable automatic reconnection behavior.
    fn set_auto_connect_suspended(&mut self, suspended: bool);
    /// Return whether auto-connect is currently suspended.
    fn is_auto_connect_suspended(&self) -> bool;
}
