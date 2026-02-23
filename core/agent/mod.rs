//! Persistent agent thread storage and indexing.
//!
//! This module keeps conversation threads durable across app restarts and
//! provides simple metadata search/filter over thread summaries.

pub mod thread_index;
pub mod thread_store;

pub use thread_index::{ThreadFilter, ThreadIndex, ThreadIndexData, ThreadSummary};
pub use thread_store::{Thread, ThreadMessage, ThreadNote, ThreadStore, TokenUsage};
