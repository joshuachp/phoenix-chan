//! Configures a [`Client`]

use std::sync::Arc;

use async_tungstenite::tokio::connect_async_with_tls_connector_and_config;
use base64::Engine;
use rustls::ClientConfig;
use tokio_rustls::TlsConnector;
use tracing::{instrument, trace};
use tungstenite::http::Uri;
use tungstenite::protocol::WebSocketConfig;
use tungstenite::ClientRequestBuilder;

use crate::{Client, Error};

/// Authentication token prefix
///
/// See <https://github.com/phoenixframework/phoenix/blob/ad1a7ee2c9c29ff102b94242fdbb9cb14dd0dd4b/assets/js/phoenix/constants.js#L30>
const AUTH_TOKEN_PREFIX: &str = "base64url.bearer.phx.";

const BASE_64: base64::engine::GeneralPurpose = base64::prelude::BASE64_URL_SAFE_NO_PAD;

/// Builder to configure a [`Client`]
#[derive(Debug)]
pub struct Builder {
    uri: Uri,
    client_req: ClientRequestBuilder,
    ws_config: WebSocketConfig,
    tls_config: Option<Arc<ClientConfig>>,
    auth_token: Option<String>,
}

impl Builder {
    /// Returns a new instance with defaults set.
    pub fn new(uri: Uri) -> Self {
        let client_req = ClientRequestBuilder::new(uri.clone()).with_sub_protocol("phoenix");

        Self {
            uri,
            client_req,
            ws_config: WebSocketConfig::default(),
            tls_config: None,
            auth_token: None,
        }
    }

    /// Configure the [`WebSocketConfig`]
    pub fn ws_config(mut self, ws_config: WebSocketConfig) -> Self {
        self.ws_config = ws_config;

        self
    }

    /// Add headers to the client connection request.
    pub fn add_header(mut self, key: String, value: String) -> Self {
        self.client_req = self.client_req.with_header(key, value);

        self
    }

    /// Add a sub-protocol header to the WebSocket connection.
    pub fn add_sub_protocol(mut self, key: String, value: String) -> Self {
        self.client_req = self.client_req.with_header(key, value);

        self
    }

    /// Set the authentication token to pass to the server.
    pub fn auth_token(mut self, token: &str) -> Self {
        let encoded = BASE_64.encode(token);

        self.auth_token = Some(format!("{AUTH_TOKEN_PREFIX}{encoded}"));

        self
    }

    /// Configure the [`WebSocketConfig`]
    pub fn tls_config(mut self, tls_config: Arc<ClientConfig>) -> Self {
        self.tls_config = Some(tls_config);

        self
    }

    /// Returns a configured client.
    #[instrument(skip(self), fields(uri = %self.uri))]
    pub async fn connect(mut self) -> Result<Client, Error> {
        if let Some(token) = self.auth_token {
            self.client_req = self.client_req.with_sub_protocol(token);
        }

        let connector = self.tls_config.map(TlsConnector::from);

        let (connection, resp) = connect_async_with_tls_connector_and_config(
            self.client_req,
            connector,
            Some(self.ws_config),
        )
        .await
        .map_err(Error::Connect)?;

        trace!(status = %resp.status(), headers = ?resp.headers());

        Ok(Client { connection })
    }
}
