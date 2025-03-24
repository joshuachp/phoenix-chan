//! Client for the Phoenix channel

use async_tungstenite::tokio::ConnectStream;
use async_tungstenite::WebSocketStream;
use tungstenite::http::Uri;

use crate::Builder;

/// Connection for the Phoenix channel
#[derive(Debug)]
pub struct Client {
    pub(crate) connection: WebSocketStream<ConnectStream>,
}

impl Client {
    /// Returns a builder to configure the client.
    pub fn builder(uri: Uri) -> Builder {
        Builder::new(uri)
    }
}
