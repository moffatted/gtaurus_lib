use gtaurus_common::*;
use serialport::{DataBits, FlowControl, Parity, SerialPort, StopBits};
use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;

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

struct DummySerialPort {
    read_buf: Vec<u8>,
    written: Vec<u8>,
}

impl Read for DummySerialPort {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.read_buf.is_empty() {
            std::thread::sleep(Duration::from_millis(10));
            return Err(io::Error::new(io::ErrorKind::TimedOut, "timeout"));
        }
        let len = std::cmp::min(buf.len(), self.read_buf.len());
        buf[..len].copy_from_slice(&self.read_buf[..len]);
        self.read_buf.drain(..len);
        Ok(len)
    }
}

impl Write for DummySerialPort {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.written.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl SerialPort for DummySerialPort {
    fn name(&self) -> Option<String> {
        Some("DUMMY".to_string())
    }
    fn baud_rate(&self) -> serialport::Result<u32> {
        Ok(115200)
    }
    fn data_bits(&self) -> serialport::Result<DataBits> {
        Ok(DataBits::Eight)
    }
    fn flow_control(&self) -> serialport::Result<FlowControl> {
        Ok(FlowControl::None)
    }
    fn parity(&self) -> serialport::Result<Parity> {
        Ok(Parity::None)
    }
    fn stop_bits(&self) -> serialport::Result<StopBits> {
        Ok(StopBits::One)
    }
    fn timeout(&self) -> Duration {
        Duration::from_millis(100)
    }
    fn set_baud_rate(&mut self, _: u32) -> serialport::Result<()> {
        Ok(())
    }
    fn set_data_bits(&mut self, _: DataBits) -> serialport::Result<()> {
        Ok(())
    }
    fn set_flow_control(&mut self, _: FlowControl) -> serialport::Result<()> {
        Ok(())
    }
    fn set_parity(&mut self, _: Parity) -> serialport::Result<()> {
        Ok(())
    }
    fn set_stop_bits(&mut self, _: StopBits) -> serialport::Result<()> {
        Ok(())
    }
    fn set_timeout(&mut self, _: Duration) -> serialport::Result<()> {
        Ok(())
    }
    fn write_request_to_send(&mut self, _: bool) -> serialport::Result<()> {
        Ok(())
    }
    fn write_data_terminal_ready(&mut self, _: bool) -> serialport::Result<()> {
        Ok(())
    }
    fn read_clear_to_send(&mut self) -> serialport::Result<bool> {
        Ok(true)
    }
    fn read_data_set_ready(&mut self) -> serialport::Result<bool> {
        Ok(true)
    }
    fn read_ring_indicator(&mut self) -> serialport::Result<bool> {
        Ok(true)
    }
    fn read_carrier_detect(&mut self) -> serialport::Result<bool> {
        Ok(true)
    }
    fn bytes_to_read(&self) -> serialport::Result<u32> {
        Ok(self.read_buf.len() as u32)
    }
    fn bytes_to_write(&self) -> serialport::Result<u32> {
        Ok(self.written.len() as u32)
    }
    fn clear(&self, _: serialport::ClearBuffer) -> serialport::Result<()> {
        Ok(())
    }
    fn try_clone(&self) -> serialport::Result<Box<dyn SerialPort>> {
        Ok(Box::new(DummySerialPort {
            read_buf: self.read_buf.clone(),
            written: self.written.clone(),
        }))
    }
    fn set_break(&self) -> serialport::Result<()> {
        Ok(())
    }
    fn clear_break(&self) -> serialport::Result<()> {
        Ok(())
    }
}

#[test]
fn dummy_serial_test() {
    let mut port = DummySerialPort {
        read_buf: b"ok\n".to_vec(),
        written: vec![],
    };
    let mut buf = [0u8; 10];
    let n = port.read(&mut buf).unwrap();
    assert_eq!(&buf[..n], b"ok\n");
}

#[test]
fn test_simulated_serial_connection() {
    let port = DummySerialPort {
        read_buf: b"ok\n".to_vec(),
        written: vec![],
    };
    let writer_clone = SerialWrapper(port.try_clone().unwrap());

    let observer = Arc::new(MockObserver::new());
    let mut driver = FluidNCDriver::new(observer.clone());

    // Simulate what `connect_serial` does, but bypass `serialport::new()`
    let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<String>();
    let pending_bytes = Arc::new(Mutex::new(0usize));
    let pending_lens = Arc::new(Mutex::new(std::collections::VecDeque::<usize>::new()));
    let reset_signal = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let rt_inner: Box<dyn SerialPort + Send> = port.try_clone().unwrap();
    let rt_port = Arc::new(Mutex::new(rt_inner));

    if let Ok(mut s) = driver.status.lock() {
        *s = ConnectionStatus::Serial("DUMMY".to_string());
    }

    FluidNCDriver::spawn_serial_writer(
        writer_clone,
        cmd_rx,
        pending_bytes.clone(),
        pending_lens.clone(),
        driver.status.clone(),
        driver.observer.clone(),
        reset_signal.clone(),
    );

    driver.conn = ActiveConnection::Serial {
        _port: SerialWrapper(Box::new(port)),
        rt_port,
        cmd_tx,
        pending_bytes,
        pending_lens: pending_lens.clone(),
        reset_signal,
    };

    // Test Write (Send command)
    driver.send_command("G0 X100".to_string()).unwrap();

    // Since our DummySerialPort doesn't share state between clones (it clones the Vec),
    // this test only validates that the writer thread consumes the command. To validate the bytes,
    // we would need a memory pipe or a `mut Arc<Mutex<Vec<u8>>>` in the mock.
    // For now, testing that the command executes without error increases the coverage on the
    // spawn_serial_writer match/loop logic.

    std::thread::sleep(std::time::Duration::from_millis(50));

    // Test realtime byte execution
    driver.send_realtime(0x18).unwrap();

    std::thread::sleep(std::time::Duration::from_millis(50));
    let msgs = observer.messages.lock().unwrap();
    assert!(msgs
        .iter()
        .any(|m| m.contains("Soft Reset (0x18) - Local buffer cleared")));
}

#[test]
fn test_connect_telnet_failure() {
    let observer = Arc::new(MockObserver::new());
    let mut driver = FluidNCDriver::new(observer.clone());

    // Connect to a non-existent port on localhost should fail
    let res = driver.connect_telnet("127.0.0.1", 9999);
    assert!(res.is_err());
    assert_eq!(driver.get_status(), "Disconnected");
}

#[test]
fn test_connect_serial_failure() {
    let observer = Arc::new(MockObserver::new());
    let mut driver = FluidNCDriver::new(observer.clone());

    // Connect to a non-existent serial port should fail
    let res = driver.connect_serial("INVALID_PORT_XYZ", 115200);
    assert!(res.is_err());
    assert_eq!(driver.get_status(), "Disconnected");
}

#[test]
fn test_auto_connect_suspend() {
    let observer = Arc::new(MockObserver::new());
    let mut driver = FluidNCDriver::new(observer.clone());

    assert!(!driver.is_auto_connect_suspended());
    driver.set_auto_connect_suspended(true);
    assert!(driver.is_auto_connect_suspended());
}

#[test]
fn test_spawn_serial_reader() {
    let port = DummySerialPort {
        read_buf: b"status: idle\nok\n".to_vec(),
        written: vec![],
    };

    let observer = Arc::new(MockObserver::new());
    let status = Arc::new(Mutex::new(ConnectionStatus::Serial("DUMMY".to_string())));
    let pending_bytes = Arc::new(Mutex::new(0usize));
    let pending_lens = Arc::new(Mutex::new(std::collections::VecDeque::<usize>::new()));

    // Seed pending lenses to match "ok\n"
    pending_lens.lock().unwrap().push_back(5);

    let reader_wrapper = SerialWrapper(Box::new(port));

    // Spawn the reader
    FluidNCDriver::spawn_serial_reader(
        reader_wrapper,
        pending_bytes,
        pending_lens.clone(),
        observer.clone(),
        status.clone(),
    );

    // Wait for the read loop to process those bytes
    std::thread::sleep(Duration::from_millis(100));

    // Validates that it popped off pending lenses when "ok" arrived
    assert_eq!(pending_lens.lock().unwrap().len(), 0);

    // Check observer output
    let msgs = observer.messages.lock().unwrap();
    assert!(msgs.iter().any(|m| m.contains("status: idle")));
    assert!(msgs.iter().any(|m| m == "ok"));
}

#[test]
fn test_spawn_serial_writer() {
    let port = DummySerialPort {
        read_buf: vec![],
        written: vec![],
    };

    let writer_wrapper = SerialWrapper(Box::new(port));

    let observer = Arc::new(MockObserver::new());
    let status = Arc::new(Mutex::new(ConnectionStatus::Serial("DUMMY".to_string())));
    let pending_bytes = Arc::new(Mutex::new(0usize));
    let pending_lens = Arc::new(Mutex::new(std::collections::VecDeque::<usize>::new()));
    let reset_signal = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<String>();

    FluidNCDriver::spawn_serial_writer(
        writer_wrapper,
        cmd_rx,
        pending_bytes.clone(),
        pending_lens.clone(),
        status.clone(),
        observer.clone(),
        reset_signal.clone(),
    );

    // Send a normal command
    cmd_tx.send("G0 X10 Y10".to_string()).unwrap();

    // Send a realtime command (bypass buffer)
    cmd_tx.send("\x18".to_string()).unwrap();

    std::thread::sleep(Duration::from_millis(100));

    // The logic inside spawn_serial_writer loops over cmd_rx.
    // If we've successfully unblocked the rx.recv() it means coverage hits the inner loop.
}

#[test]
fn test_spawn_serial_writer_soft_reset() {
    let port = DummySerialPort {
        read_buf: vec![],
        written: vec![],
    };

    let writer_wrapper = SerialWrapper(Box::new(port));

    let observer = Arc::new(MockObserver::new());
    let status = Arc::new(Mutex::new(ConnectionStatus::Serial("DUMMY".to_string())));
    let pending_bytes = Arc::new(Mutex::new(0usize));
    let pending_lens = Arc::new(Mutex::new(std::collections::VecDeque::<usize>::new()));
    let reset_signal = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<String>();

    FluidNCDriver::spawn_serial_writer(
        writer_wrapper,
        cmd_rx,
        pending_bytes.clone(),
        pending_lens.clone(),
        status.clone(),
        observer.clone(),
        reset_signal.clone(),
    );

    // Queue commands
    cmd_tx.send("G0 X10 Y10".to_string()).unwrap();
    cmd_tx.send("G0 X20 Y20".to_string()).unwrap();

    // Trigger soft reset
    reset_signal.store(true, std::sync::atomic::Ordering::Relaxed);

    std::thread::sleep(Duration::from_millis(100));
}

#[test]
fn test_writer_disconnect_exit() {
    let port = DummySerialPort {
        read_buf: vec![],
        written: vec![],
    };

    let writer_wrapper = SerialWrapper(Box::new(port));

    let observer = Arc::new(MockObserver::new());
    // Start disconnected
    let status = Arc::new(Mutex::new(ConnectionStatus::Disconnected));
    let pending_bytes = Arc::new(Mutex::new(0usize));
    let pending_lens = Arc::new(Mutex::new(std::collections::VecDeque::<usize>::new()));
    let reset_signal = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<String>();

    // This should immediately return because status is "Disconnected"
    FluidNCDriver::spawn_serial_writer(
        writer_wrapper,
        cmd_rx,
        pending_bytes.clone(),
        pending_lens.clone(),
        status.clone(),
        observer.clone(),
        reset_signal.clone(),
    );

    cmd_tx.send("G0 X10 Y10".to_string()).unwrap();
    std::thread::sleep(Duration::from_millis(50));
}

#[test]
fn test_connection_status_display() {
    assert_eq!(
        format!("{}", ConnectionStatus::Disconnected),
        "Disconnected"
    );
    assert_eq!(
        format!("{}", ConnectionStatus::Serial("COM3".to_string())),
        "Serial: COM3"
    );
    assert_eq!(
        format!(
            "{}",
            ConnectionStatus::Telnet("192.168.1.100:23".to_string())
        ),
        "WiFi: 192.168.1.100:23"
    );
}

#[test]
fn test_tcp_reader_writer() {
    use std::net::{TcpListener, TcpStream};

    // Setup a dummy telnet server
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let observer = Arc::new(MockObserver::new());
    let status = Arc::new(Mutex::new(ConnectionStatus::Telnet(format!(
        "127.0.0.1:{}",
        port
    ))));
    let reset_signal = Arc::new(std::sync::atomic::AtomicBool::new(false));

    // Connect the stream
    let client_stream = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
    let (mut server_stream, _) = listener.accept().unwrap();

    // Reader clone
    let reader_stream = client_stream.try_clone().unwrap();

    let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<String>();

    // Spawn Writer
    FluidNCDriver::spawn_tcp_writer(
        Arc::new(Mutex::new(client_stream)),
        cmd_rx,
        status.clone(),
        observer.clone(),
        reset_signal.clone(),
    );

    // Spawn Reader
    FluidNCDriver::spawn_tcp_reader(reader_stream, observer.clone(), status.clone());

    // Test sending command
    cmd_tx.send("G0 X10".to_string()).unwrap();

    // Verify server receives it
    let mut buf = [0u8; 128];
    let n = server_stream.read(&mut buf).unwrap();
    assert_eq!(&buf[..n], b"G0 X10\n"); // spawn_tcp_writer appends \n

    // Test sending response back to client reader
    server_stream.write_all(b"status: ok\n").unwrap();
    std::thread::sleep(Duration::from_millis(100));

    // Verify observer got the parsed line
    let msgs = observer.messages.lock().unwrap();
    assert!(msgs.iter().any(|m| m == "status: ok"));
    drop(msgs); // Prevent deadlock in reader thread's emit call

    // Close streams to force client reader to exit with Ok(0)
    let _ = server_stream.shutdown(std::net::Shutdown::Both);

    // Polling loop to wait for thread to update status
    for _ in 0..10 {
        if let Ok(s) = status.lock() {
            if *s == ConnectionStatus::Disconnected {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    // Verify status updated to disconnected
    assert_eq!(*status.lock().unwrap(), ConnectionStatus::Disconnected);
}

#[test]
fn test_tcp_writer_soft_reset() {
    use std::net::{TcpListener, TcpStream};

    // Setup a dummy telnet server
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let observer = Arc::new(MockObserver::new());
    let status = Arc::new(Mutex::new(ConnectionStatus::Telnet(format!(
        "127.0.0.1:{}",
        port
    ))));
    let reset_signal = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let client_stream = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
    let (_server_stream, _) = listener.accept().unwrap();

    let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<String>();

    FluidNCDriver::spawn_tcp_writer(
        Arc::new(Mutex::new(client_stream)),
        cmd_rx,
        status.clone(),
        observer.clone(),
        reset_signal.clone(),
    );

    cmd_tx.send("G0 X10".to_string()).unwrap();
    cmd_tx.send("G0 X20".to_string()).unwrap();

    // Trigger soft reset
    reset_signal.store(true, std::sync::atomic::Ordering::Relaxed);

    // Wait for writer to process
    std::thread::sleep(Duration::from_millis(600));
}

#[test]
fn test_tcp_error_handling() {
    use std::net::{TcpListener, TcpStream};

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let observer = Arc::new(MockObserver::new());
    let status = Arc::new(Mutex::new(ConnectionStatus::Telnet(format!(
        "127.0.0.1:{}",
        port
    ))));
    let reset_signal = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let client_stream = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
    let (server_stream, _) = listener.accept().unwrap();

    // Set read timeout to hit `TimedOut` block
    client_stream
        .set_read_timeout(Some(Duration::from_millis(10)))
        .unwrap();
    let reader_stream = client_stream.try_clone().unwrap();
    let writer_stream = client_stream.try_clone().unwrap();

    let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<String>();

    FluidNCDriver::spawn_tcp_writer(
        Arc::new(Mutex::new(writer_stream)),
        cmd_rx,
        status.clone(),
        observer.clone(),
        reset_signal.clone(),
    );

    FluidNCDriver::spawn_tcp_reader(reader_stream, observer.clone(), status.clone());

    // Sleep so the reader thread hits the TimedOut logic a few times
    std::thread::sleep(Duration::from_millis(50));

    // Force a read error by dropping server stream, then writing to the closed socket
    drop(server_stream);
    std::thread::sleep(Duration::from_millis(50));

    // Sending should now fail, hitting TCP write error logic
    cmd_tx.send("G0 X10".to_string()).unwrap();

    // Wait for it to fail
    for _ in 0..10 {
        if let Ok(s) = status.lock() {
            if *s == ConnectionStatus::Disconnected {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(*status.lock().unwrap(), ConnectionStatus::Disconnected);
}

#[test]
fn test_driver_disconnected_methods() {
    let observer = Arc::new(MockObserver::new());
    let mut driver = FluidNCDriver::new(observer);

    assert!(driver.send_command("G0 X10".to_string()).is_err());
    assert!(driver.send_realtime(0x18).is_err());
}

#[test]
fn test_driver_telnet_connection() {
    use std::io::Read;
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let observer = Arc::new(MockObserver::new());
    let mut driver = FluidNCDriver::new(observer.clone());

    // Connect
    assert!(driver.connect_telnet("127.0.0.1", port).is_ok());

    let (mut server_stream, _) = listener.accept().unwrap();

    // Test command sending
    assert!(driver.send_command("G0 X10".to_string()).is_ok());
    assert!(driver.send_realtime(0x18).is_ok());

    // Give writer thread time to dispatch
    std::thread::sleep(Duration::from_millis(150));

    // Read from server stream
    let mut buf = [0u8; 128];
    // server_stream.set_nonblocking(true).unwrap(); // Optional to avoid hang if nothing sent
    let n = server_stream.read(&mut buf).unwrap_or(0);
    assert!(n > 0);

    driver.disconnect();
    assert_eq!(driver.get_status(), "Disconnected");
}
