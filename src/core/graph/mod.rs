//! Graph module for deadlock detection
//!
//! This module contains graph implementations used for deadlock detection:
//! - Wait-for graph: tracks which threads are waiting for which other threads
//! - Lock order graph: tracks the order in which locks are acquired (optional feature)

#[cfg(feature = "lock-order-graph")]
pub(crate) mod lock_order_graph;
pub(crate) mod wait_for_graph;

#[cfg(feature = "lock-order-graph")]
pub(crate) use lock_order_graph::LockOrderGraph;
pub(crate) use wait_for_graph::WaitForGraph;
