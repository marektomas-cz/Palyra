//! Persisted state and profile/download storage internals for browserd.

pub(crate) mod profile_registry;
pub(crate) mod state_store;

pub(crate) use profile_registry::*;
pub(crate) use state_store::*;
