//! Core domain types and pure logic for pulse-sync.
//!
//! Nothing in this crate talks to the network or the database, which keeps the
//! hardest parts of syncing (versioning, chunking) easy to test in isolation.

pub mod chunker;
pub mod events;
pub mod ids;
pub mod model;
pub mod time;
pub mod version_vector;

pub use ids::{DeviceId, FolderId};
pub use version_vector::{VersionOrdering, VersionVector};
