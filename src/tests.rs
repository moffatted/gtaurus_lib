#[cfg(test)]
mod tests {
    use crate::types::*;
    use crate::driver::*;
    use crate::traits::*;
    use std::sync::{Arc, Mutex, mpsc};

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
    fn test_connection_status_display() {
        assert_eq!(ConnectionStatus::Disconnected.to_string(), "Disconnected");
        assert_eq!(
            ConnectionStatus::Serial("COM3".to_string()).to_string(),
            "Serial: COM3"
        );
        assert_eq!(
            ConnectionStatus::Telnet("192.168.1.100:23".to_string()).to_string(),
            "WiFi: 192.168.1.100:23"
        );
    }

    #[test]
    fn test_fluidnc_driver_initialization() {
        let observer = Arc::new(MockObserver::new());
        let driver = FluidNCDriver::new(observer);

        assert_eq!(driver.get_status(), "Disconnected");
        assert!(!driver.is_auto_connect_suspended());
        let status = driver.status.lock().unwrap();
        assert_eq!(*status, ConnectionStatus::Disconnected);
    }

    #[test]
    fn test_fluidnc_driver_suspend_auto_connect() {
        let observer = Arc::new(MockObserver::new());
        let mut driver = FluidNCDriver::new(observer);

        driver.set_auto_connect_suspended(true);
        assert!(driver.is_auto_connect_suspended());

        driver.set_auto_connect_suspended(false);
        assert!(!driver.is_auto_connect_suspended());
    }

    #[test]
    fn test_send_command_disconnected() {
        let observer = Arc::new(MockObserver::new());
        let mut driver = FluidNCDriver::new(observer);

        let result = driver.send_command("G0 X10".to_string());
        assert_eq!(result.unwrap_err(), "Not connected");
    }

    #[test]
    fn test_send_realtime_disconnected() {
        let observer = Arc::new(MockObserver::new());
        let mut driver = FluidNCDriver::new(observer);

        let result = driver.send_realtime(0x85); // Jog Cancel
        assert_eq!(result.unwrap_err(), "Not connected");
    }

    #[test]
    fn test_add_rx_subscriber() {
        let observer = Arc::new(MockObserver::new());
        let mut driver = FluidNCDriver::new(observer);

        let (tx, _rx) = mpsc::channel();
        driver.add_rx_subscriber(tx);

        let subs = driver.subscribers.lock().unwrap();
        assert_eq!(subs.len(), 1);
    }
}
