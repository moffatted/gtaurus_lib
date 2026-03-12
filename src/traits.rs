/*
 * @file traits.rs
 * @purpose Trait definitions for machine connections, enabling interchangeable communication backends.
 * @author Ed Moffatt
 */
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
