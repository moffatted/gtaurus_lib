//! Command queueing for both serial and TCP connections.

use crate::types::ActiveConnection;

/// Queue a G-code command for transmission to the machine.
///
/// Sends the command to the appropriate writer thread based on the active connection type.
/// The actual transmission happens asynchronously.
///
/// # Arguments
///
/// * `conn` - The active connection (serial or telnet)
/// * `cmd` - The G-code command string to send
///
/// # Errors
///
/// Returns an error if:
/// - No connection is active (`ActiveConnection::None`)
/// - The writer thread has crashed (channel is closed)
/// - The command cannot be serialized
///
/// # Example
///
/// ```ignore
/// use gtaurus_lib::driver::ActiveConnection;
/// use gtaurus_lib::transport::command::send_queued_command;
///
/// // Assuming conn is properly initialized...
/// send_queued_command(&conn, "G0 X10".to_string())?;
/// ```
pub fn send_queued_command(conn: &ActiveConnection, cmd: String) -> Result<(), String> {
    match conn {
        ActiveConnection::Serial { cmd_tx, .. } => cmd_tx.send(cmd).map_err(|e| e.to_string()),
        ActiveConnection::Telnet { cmd_tx, .. } => cmd_tx.send(cmd).map_err(|e| e.to_string()),
        ActiveConnection::None => Err("Not connected".to_string()),
    }
}
