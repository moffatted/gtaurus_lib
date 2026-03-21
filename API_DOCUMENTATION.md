# GTaurus Library API Documentation

## Scope

This document reflects the current public API in `gtaurus_common` (workspace folder `gtaurus_lib`).

## Public Modules

- `traits`
- `types`
- `driver`
- `transport`

## Core Trait: `GCodeConnection`

Defined in `src/traits.rs`.

```rust
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
```

### Usage Example

```rust
use std::sync::Arc;
use gtaurus_common::{DriverEventObserver, FluidNCDriver, GCodeConnection};

struct Observer;
impl DriverEventObserver for Observer {
    fn emit(&self, line: &str) {
        println!("{}", line);
    }
}

fn main() -> Result<(), String> {
    let mut driver = FluidNCDriver::new(Arc::new(Observer));
    driver.connect_serial("/dev/ttyUSB0", 115200)?;
    driver.send_command("$I".to_string())?;
    driver.send_realtime(0x3F)?; // Status query
    driver.disconnect();
    Ok(())
}
```

## Primary Driver Type

`FluidNCDriver` is defined in `src/driver.rs` and implements `GCodeConnection`.

Important fields (public):
- `conn: ActiveConnection`
- `status: Arc<Mutex<ConnectionStatus>>`
- `observer: Arc<dyn DriverEventObserver>`
- `auto_connect_suspended: bool`
- `subscribers: Arc<Mutex<Vec<Sender<String>>>>`

## Core Types

Defined in `src/types.rs`.

- `MAX_BUFFER_SIZE: usize = 127`
- `RX_EVENT: &str = "rx_event"`
- `SerialWrapper(pub Box<dyn SerialPort>)`
- `ConnectionStatus`:
  - `Disconnected`
  - `Serial(String)`
  - `Telnet(String)`
- `DriverEventObserver` trait
- `ActiveConnection` enum for active serial/telnet resources

## Transport Functions

### `transport::command`
- `send_queued_command(conn, cmd)`

### `transport::realtime`
- `send_serial_realtime(...)`
- `send_telnet_realtime(...)`

### `transport::serial`
- `spawn_serial_reader(...)`
- `spawn_serial_writer(...)`

### `transport::tcp`
- `spawn_tcp_reader(...)`
- `spawn_tcp_writer(...)`

## Behavior Notes

- Serial path uses character-counting flow control (`MAX_BUFFER_SIZE` budget).
- Telnet path sends queued commands directly without serial byte-budget accounting.
- Realtime channel bypasses queued command path.
- Soft reset (`0x18`) clears local pending state and signals writer drain.

## Error Model

- Public APIs primarily return `Result<_, String>`.
- Worker spawn functions generally do not return errors directly; failures are handled in worker loops and reflected through status/events.

## Validation Status

This API set is currently validated in workspace with:
- `cargo test`
- `cargo doc --no-deps`
