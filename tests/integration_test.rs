use gtaurus_common::*;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
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

#[test]
fn test_tcp_connection_and_communication() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let observer = Arc::new(MockObserver::new());
    let mut driver = FluidNCDriver::new(observer.clone());

    // Connect Telnet
    let res = driver.connect_telnet("127.0.0.1", port);
    assert!(res.is_ok(), "Failed to connect to local test server");

    // Accept connection on the mock server
    let (mut socket, _) = listener.accept().unwrap();

    // Add RX subscriber to receive parsed messages
    let (tx, _rx) = mpsc::channel();
    driver.add_rx_subscriber(tx);

    // Test Write (Send command)
    driver.send_command("G0 X100".to_string()).unwrap();
    let mut buf = [0; 64];
    thread::sleep(Duration::from_millis(50));
    let n = socket.read(&mut buf).unwrap();
    let received = String::from_utf8_lossy(&buf[..n]);
    assert_eq!(received, "G0 X100\n");

    // Test Read (Simulate incoming data from fluidNC)
    socket.write_all(b"ok\n").unwrap();
    thread::sleep(Duration::from_millis(150)); // Needs robust sleep to wait for buffer read

    let msgs = observer.messages.lock().unwrap();
    assert!(msgs.contains(&"ok".to_string()));
    drop(msgs);

    // Test Realtime byte (e.g. Pause 0x21)
    driver.send_realtime(0x21).unwrap();
    thread::sleep(Duration::from_millis(50));
    let n = socket.read(&mut buf).unwrap();
    assert_eq!(&buf[..1], &[0x21]);

    // Test Soft Reset (0x18)
    driver.send_realtime(0x18).unwrap();
    thread::sleep(Duration::from_millis(50));
    let n = socket.read(&mut buf).unwrap();
    assert_eq!(&buf[..1], &[0x18]);

    // Ensure observer emitted soft reset log
    let msgs = observer.messages.lock().unwrap();
    assert!(msgs.iter().any(|m| m.contains("Soft Reset")));

    driver.disconnect();
    assert_eq!(driver.get_status(), "Disconnected");
}
