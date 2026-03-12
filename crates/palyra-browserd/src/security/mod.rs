//! Security policy internals for browserd.

pub(crate) mod auth;
pub(crate) mod target_validation;

pub(crate) use target_validation::*;
