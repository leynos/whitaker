// aux-build: async_trait.rs
//! UI fixture: ignore trait-level async-trait attributes for method docs.
#![warn(function_attrs_follow_docs)]

extern crate async_trait;
use async_trait::async_trait;

#[async_trait]
#[expect(
    dead_code,
    reason = "fixture defines an async trait without using it"
)]
pub trait MessageRepository {
    /// Checks if a message with the given ID already exists.
    async fn exists(&self, id: u64) -> bool;
}

fn main() {}
