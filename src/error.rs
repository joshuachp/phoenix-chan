//! Errors returned by the client.

use crate::message::Message;

/// Error returned by the [`Client`](crate::client::Client) or connection.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Couldn't connect to the web-socket
    #[error("couldn't connect to the web-socket")]
    Connect(#[source] tungstenite::Error),
    /// Couldn't serialize message
    #[error("couldn't serialize message")]
    Serialize(#[source] serde_json::Error),
    /// Couldn't send a message
    #[error("couldn't send message {msg}")]
    Send {
        /// The message that was sent
        msg: Message,
        #[source]
        /// Backtrace error
        backtrace: tungstenite::Error,
    },
}
