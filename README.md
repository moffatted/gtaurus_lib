# Gtaurus Core Library (`gtaurus_lib`)

This is the core CNC driver and protocol logic used by both the Gtaurus Desktop App (Tauri) and the Gtaurus Standalone Server.

## Key Features

- **Character Counting Protocol**: Implements the FluidNC/Grbl character counting protocol for high-speed, reliable streaming over buffered serial/TCP connections.
- **FluidNC Driver**: A robust abstraction for connecting to, controlling, and monitoring FluidNC-based CNC machines.
- **Dual Transport Support**: Built-in support for both Local Serial (USB) and Network (Telnet/TCP) connections.
- **Real-time Monitoring**: Asynchronous status polling and event emission for coordinates, overrides, and machine states.

## 🧪 Testing

The library includes a comprehensive suite of unit and integration tests covering:

- Protcol parser (`gtaurus_common::parser`)
- Streamer logic and character counting
- Connection lifecycle and error handling
- Serial and TCP transport layers (using simulated hardware)

### Running Tests

To run the standard test suite:

```bash
cargo test
```

To run with full output capturing (useful for debugging):

```bash
cargo test -- --nocapture
```

### Test Coverage

We use `cargo-tarpaulin` to measure code coverage.

#### Current Coverage: ~73%

To generate a coverage report locally:

1. [Install Tarpaulin](https://github.com/xd009642/cargo-tarpaulin)
2. Run:

   ```bash
   cargo tarpaulin
   ```

*Note: The remaining ~27% of code paths primarily involve specific hardware-level failure modes during the initial serial handshake, which are difficult to mock reliably on all OS platforms without physical hardware loopbacks.*
