//! Client library for Phoenix channels.

#![warn(
    missing_docs,
    rustdoc::missing_crate_level_docs,
    clippy::todo,
    rustdoc::broken_intra_doc_links
)]

pub mod builder;
pub mod client;
pub mod error;

pub use self::builder::Builder;
pub use self::client::Client;
pub use self::error::Error;

// pub dependencies
pub use rustls;
pub use tungstenite;
