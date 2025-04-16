//! Errors returned by the client.

use tungstenite::http;

use crate::message::Message;

type TungsteniteError = Box<tungstenite::Error>;

/// Error returned by the [`Client`](crate::client::Client) or connection.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Couldn't add headers to uri.
    #[error("couldn't add the vsn header to uri")]
    Uri(#[source] http::uri::InvalidUri),
    /// Couldn't add headers to uri.
    #[error("couldn't build the uri with the vsn version")]
    UriBuild(#[source] http::Error),
    /// Couldn't connect to the web-socket
    #[error("couldn't connect to the web-socket")]
    Connect(#[source] TungsteniteError),
    /// Couldn't serialize message
    #[error("couldn't serialize message")]
    Serialize(#[source] serde_json::Error),
    /// Couldn't de-serialize message
    #[error("couldn't deserialize message")]
    Deserialize(#[source] serde_json::Error),
    /// Couldn't send a message
    #[error("couldn't send message {msg}")]
    Send {
        /// The message that was sent
        msg: Message<()>,
        #[source]
        /// Backtrace error
        backtrace: TungsteniteError,
    },
    /// Couldn't receive the message
    #[error("couldn't receive the message")]
    Recv(#[source] TungsteniteError),
    /// Couldn't decode WebSocket message, not of type text
    #[error("couldn't decode websocket message, not of type text")]
    WebSocketMessageType(#[source] TungsteniteError),
    /// Disconnected from the web socket
    #[error("the web-socket disconnected")]
    Disconnected,
}
