//! Configures a [`Client`]

use std::sync::Arc;
use std::time::Duration;

use async_tungstenite::tokio::connect_async_with_tls_connector_and_config;
use base64::Engine;
use rustls::ClientConfig;
use tokio_rustls::TlsConnector;
use tracing::trace;
use tungstenite::http::uri::PathAndQuery;
use tungstenite::http::Uri;
use tungstenite::protocol::WebSocketConfig;
use tungstenite::ClientRequestBuilder;

use crate::{Client, Error};

/// Authentication token prefix
///
/// See <https://github.com/phoenixframework/phoenix/blob/ad1a7ee2c9c29ff102b94242fdbb9cb14dd0dd4b/assets/js/phoenix/constants.js#L30>
const AUTH_TOKEN_PREFIX: &str = "base64url.bearer.phx.";

const BASE_64: base64::engine::GeneralPurpose = base64::prelude::BASE64_URL_SAFE_NO_PAD;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_HEARTBEAT: Duration = Duration::from_secs(DEFAULT_TIMEOUT.as_secs() / 2);

/// Builder to configure a [`Client`]
#[derive(Debug)]
pub struct Builder {
    client_req: ClientRequestBuilder,
    ws_config: WebSocketConfig,
    tls_config: Option<Arc<ClientConfig>>,
    auth_token: Option<String>,
    heartbeat: Duration,
}

impl Builder {
    /// Returns a new instance with defaults set.
    #[must_use]
    pub fn new(mut uri: Uri) -> Result<Self, Error> {
        let has_vsn = uri
            .query()
            .is_some_and(|s| s.split('&').any(|s| s.starts_with("vsn=")));

        if !has_vsn {
            let pq = match uri.query() {
                Some(query) if !query.is_empty() => {
                    PathAndQuery::try_from(format!("{}?{query}&vsn=2.0.0", uri.path()))
                        .map_err(Error::Uri)?
                }
                Some(_) | None => PathAndQuery::try_from(format!("{}?vsn=2.0.0", uri.path()))
                    .map_err(Error::Uri)?,
            };

            uri = tungstenite::http::uri::Builder::from(uri)
                .path_and_query(pq)
                .build()
                .map_err(Error::UriBuild)?;
        }

        let client_req = ClientRequestBuilder::new(uri.clone());

        Ok(Self {
            client_req,
            ws_config: WebSocketConfig::default(),
            tls_config: None,
            auth_token: None,
            // https://github.com/phoenixframework/phoenix/blob/ad1a7ee2c9c29ff102b94242fdbb9cb14dd0dd4b/assets/js/phoenix/constants.js#L6
            heartbeat: DEFAULT_HEARTBEAT,
        })
    }

    /// Configure the [`WebSocketConfig`]
    #[must_use]
    pub fn ws_config(mut self, ws_config: WebSocketConfig) -> Self {
        self.ws_config = ws_config;

        self
    }

    /// Add headers to the client connection request.
    #[must_use]
    pub fn add_header(mut self, key: String, value: String) -> Self {
        self.client_req = self.client_req.with_header(key, value);

        self
    }

    /// Add a sub-protocol header to the WebSocket connection.
    #[must_use]
    pub fn add_sub_protocol(mut self, key: String, value: String) -> Self {
        self.client_req = self.client_req.with_header(key, value);

        self
    }

    /// Set the authentication token to pass to the server.
    #[must_use]
    pub fn auth_token(mut self, token: &str) -> Self {
        let encoded = BASE_64.encode(token);

        self.auth_token = Some(format!("{AUTH_TOKEN_PREFIX}{encoded}"));

        self.client_req = self.client_req.with_sub_protocol("phoenix");

        self
    }

    /// Configure the [`WebSocketConfig`]
    #[must_use]
    pub fn tls_config(mut self, tls_config: Arc<ClientConfig>) -> Self {
        self.tls_config = Some(tls_config);

        self
    }

    /// Set the heart-bit interval duration.
    #[must_use]
    pub fn heartbeat(mut self, heartbeat: Duration) -> Self {
        self.heartbeat = heartbeat;

        self
    }

    /// Returns a configured client.
    #[must_use]
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

        Ok(Client::new(connection, self.heartbeat))
    }
}
