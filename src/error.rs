//! Errors returned by the client.

/// Error returned by the [`Client`](crate::client::Client) or connection.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Couldn't connect to the web-socket
    #[error("couldn't connect to the web-socket")]
    Connect(#[source] tungstenite::Error),
}
