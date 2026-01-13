//! Stateless validator common types and utilities.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod execution_payload;
pub mod guest;

#[cfg(feature = "host")]
pub mod host;
