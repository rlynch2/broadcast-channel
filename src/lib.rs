//! A broadcasting channel
//!
//! This crate provides structs to broadcast messages to multiple receivers,
//! unlike a mpmc channel, every receiver will receive all messages sent after its creation.
//!
//! The two types used in the channel are:   
//!
//! * [`Sender`]
//! * [`Receiver`]  
//!
//! [`Sender`]: struct.Sender.html
//! [`Receiver`]: struct.Receiver.html
//!
//! # Examples  
//! Simple use:
//! ```rust
//! use broadcast_channel::broadcaster;
//!
//! # fn main() {
//! let (tx, mut rx) = broadcaster();
//! tx.send(1);
//! assert_eq!(rx.next(), Some(1));   
//! # }
//! ```
//! Threaded use:
//!```rust
//! use broadcast_channel::broadcaster;
//! use std::thread;
//!
//! # fn main() {
//! let (tx, mut rx) = broadcaster();
//! let thread = thread::spawn(move || {
//!     tx.send_all(0..10);
//! });
//! thread.join();
//! for (i, item) in rx.enumerate() {
//!     assert_eq!(i, item);
//! }
//! # }

mod sync;
pub use sync::*;
