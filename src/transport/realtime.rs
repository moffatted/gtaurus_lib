//! Realtime command transmission (high-priority, queue-bypassing control messages).
//!
//! Realtime commands are sent on a separate channel that bypasses the normal command queue.
//! This allows immediate transmission of control messages like feed hold and soft reset.

use std::collections::VecDeque;
use std::io::Write;
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serialport::SerialPort;

use crate::types::DriverEventObserver;

/// Send a realtime command to a machine via serial port.
///
/// Realtime commands bypass the normal command queue. Used for time-sensitive
/// operations like feed hold, cycle start, and most importantly soft reset.
///
/// # Arguments
///
/// * `rt_port` - Shared mutable reference to the serial port
/// * `pending_bytes` - Current count of pending characters in machine's buffer
/// * `pending_lens` - Queue of command lengths (popped when "ok" received)
/// * `reset_signal` - Flag to signal writer thread to drain pending commands
/// * `observer` - Event observer for emitting debug output
/// * `byte` - The realtime command byte to send
///
/// # Errors
///
/// Returns an error if:
/// - The mutex is poisoned (corrupted by thread panic)
/// - The serial write fails
/// - The flush fails
///
/// # Special Handling for Soft Reset (0x18)
///
/// When the soft reset byte (0x18) is sent:
/// 1. All local pending bytes are cleared immediately
/// 2. The writer thread is signaled to drain any queued commands
/// 3. Observer receives debug output indicating the reset
///
/// # Panics
///
/// Can panic if acquiring the lock fails due to poisoning.
/// This should be caught and handled as a fatal error.
pub fn send_serial_realtime(
    rt_port: &Arc<Mutex<Box<dyn SerialPort + Send>>>,
    pending_bytes: &Arc<Mutex<usize>>,
    pending_lens: &Arc<Mutex<VecDeque<usize>>>,
    reset_signal: &Arc<AtomicBool>,
    observer: &Arc<dyn DriverEventObserver>,
    byte: u8,
) -> Result<(), String> {
    {
        let mut p = rt_port.lock().map_err(|_| "Poisoned".to_string())?;
        p.write_all(&[byte]).map_err(|e| e.to_string())?;
        p.flush().map_err(|e| e.to_string())?;
    }

    if byte == 0x18 {
        *pending_bytes.lock().unwrap() = 0;
        pending_lens.lock().unwrap().clear();
        reset_signal.store(true, Ordering::Relaxed);
        observer.emit("[GTaurus] Soft Reset (0x18) - Local buffer cleared, writer drain signaled");
    }

    Ok(())
}

/// Send a realtime command to a machine via Telnet (TCP).
///
/// Similar to serial realtime, but uses TCP stream instead of serial port.
///
/// # Arguments
///
/// * `rt_stream` - Shared mutable reference to the TCP stream
/// * `reset_signal` - Flag to signal writer thread to drain pending commands
/// * `observer` - Event observer for emitting debug output
/// * `byte` - The realtime command byte to send
///
/// # Errors
///
/// Returns an error if:
/// - The mutex is poisoned (corrupted by thread panic)
/// - The TCP write fails
/// - The flush fails
///
/// # Special Handling for Soft Reset (0x18)
///
/// When the soft reset byte (0x18) is sent, the writer thread is signaled
/// to drain any queued commands. Note: Unlike serial, there's no local buffer
/// to clear since TCP doesn't implement character-counting flow control
/// (FluidNC's WiFi module doesn't support it).
pub fn send_telnet_realtime(
    rt_stream: &Arc<Mutex<TcpStream>>,
    reset_signal: &Arc<AtomicBool>,
    observer: &Arc<dyn DriverEventObserver>,
    byte: u8,
) -> Result<(), String> {
    let mut s = rt_stream.lock().map_err(|_| "Poisoned".to_string())?;
    s.write_all(&[byte]).map_err(|e| e.to_string())?;
    s.flush().map_err(|e| e.to_string())?;

    if byte == 0x18 {
        reset_signal.store(true, Ordering::Relaxed);
        observer.emit("[GTaurus] Soft Reset (0x18) - Writer drain signaled");
    }

    Ok(())
}
