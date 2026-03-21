//! Transport layer protocols for machine communication.
//!
//! This module provides low-level implementations for serial and TCP communication.
//! Each transport spawns independent reader and writer threads that handle I/O asynchronously.
//!
//! ## Threading Model
//!
//! - **Reader Thread**: Continuously reads lines from the connection and broadcasts them to subscribers
//! - **Writer Thread**: Pulls commands from a channel and sends them respecting flow control limits
//! - **Realtime Channel**: Separate priority channel for immediate control inputs (feed hold, soft reset, etc.)
//!
//! ## Flow Control
//!
//! Both serial and TCP implementations use character-counting flow control:
//! 1. Command is sent to machine (characters added to `pending_bytes`)
//! 2. Reader waits for "ok" response from machine
//! 3. On "ok", bytes are removed from `pending_bytes`
//! 4. Writer only sends next command when `pending_bytes + new_command_length <= MAX_BUFFER_SIZE`

pub mod command;
pub mod serial;
pub mod tcp;
pub mod realtime;
