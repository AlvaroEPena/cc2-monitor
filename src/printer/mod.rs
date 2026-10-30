//! Everything that speaks the CC2 protocol. No UI code lives here.

pub mod client;
pub mod codes;
pub mod discovery;
pub mod state;
pub mod upload;

pub use client::{ConnectConfig, Dir, Event, LogEntry, Phase, PrinterClient};
pub use state::PrinterState;
