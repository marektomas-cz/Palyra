//! Shared daemon channel application payload builders.
//!
//! Generic admin and console assembly stays here while provider-owned behavior
//! is delegated to the provider registry under [`providers`].

pub(crate) mod messages;
pub(crate) mod providers;
pub(crate) mod status;

pub(crate) use messages::{build_channel_test_payload, build_channel_test_send_payload};
pub(crate) use status::{build_channel_health_refresh_payload, build_channel_status_payload};
