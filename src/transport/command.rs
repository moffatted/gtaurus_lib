use crate::types::ActiveConnection;

pub fn send_queued_command(conn: &ActiveConnection, cmd: String) -> Result<(), String> {
    match conn {
        ActiveConnection::Serial { cmd_tx, .. } => cmd_tx.send(cmd).map_err(|e| e.to_string()),
        ActiveConnection::Telnet { cmd_tx, .. } => cmd_tx.send(cmd).map_err(|e| e.to_string()),
        ActiveConnection::None => Err("Not connected".to_string()),
    }
}
