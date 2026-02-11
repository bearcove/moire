//! Roam diagnostics integration for peeps.
//!
//! Re-exports snapshot types from peeps-types. Roam-session and roam-shm
//! register themselves as diagnostics sources via `inventory::submit!`.

pub use peeps_types::{
    collect_all_diagnostics,
    // Session types
    ChannelCreditSnapshot,
    ChannelDir,
    // SHM types
    ChannelQueueSnapshot,
    ChannelSnapshot,
    CompletionSnapshot,
    ConnectionSnapshot,
    // Diagnostics registry
    Diagnostics,
    DiagnosticsSource,
    Direction,
    RequestSnapshot,
    SessionSnapshot,
    ShmPeerSnapshot,
    ShmPeerState,
    ShmSegmentSnapshot,
    ShmSnapshot,
    TransportStats,
    VarSlotClassSnapshot,
};
