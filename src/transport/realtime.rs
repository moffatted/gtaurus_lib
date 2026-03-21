use std::collections::VecDeque;
use std::io::Write;
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serialport::SerialPort;

use crate::types::DriverEventObserver;

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
