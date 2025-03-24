//! Client for the Phoenix channel

use std::borrow::Cow;

use async_tungstenite::tokio::ConnectStream;
use async_tungstenite::WebSocketStream;
use tungstenite::http::Uri;
use tungstenite::Message;

use crate::message::ChannelMsg;
use crate::{Builder, Error, Map};

type Id = usize;

/// Connection for the Phoenix channel
#[derive(Debug)]
pub struct Client {
    msg_id: usize,
    connection: WebSocketStream<ConnectStream>,
}

impl Client {
    pub(crate) fn new(connection: WebSocketStream<ConnectStream>) -> Self {
        Self {
            msg_id: 0,
            connection,
        }
    }

    /// Returns a builder to configure the client.
    pub fn builder(uri: Uri) -> Builder {
        Builder::new(uri)
    }

    /// Joins a channel.
    pub async fn join(&mut self, topic: &str) -> Result<Id, Error> {
        self.join_with_params(topic, Map::default()).await
    }

    /// Joins a channel with additional parameters.
    pub async fn join_with_params(&mut self, topic: &str, params: Map) -> Result<Id, Error> {
        let id = self.msg_id.wrapping_add(1);

        let msg = ChannelMsg::<Cow<str>> {
            join_reference: Some(Cow::Owned(id.to_string())),
            message_reference: Cow::Owned(id.to_string()),
            topic_name: Cow::Borrowed(topic),
            event_name: Cow::Borrowed("phx_join"),
            payload: params,
        };
        let msg_json = serde_json::to_string(&msg).map_err(Error::Serialize)?;

        self.connection
            .send(Message::Text(msg_json.into()))
            .await
            .map_err(|err| Error::Send {
                msg: msg.into(),
                backtrace: err,
            })?;

        Ok(id)
    }

    /// Leaves a channel.
    pub async fn leave(&mut self, topic: &str) -> Result<Id, Error> {
        self.send(topic, "phx_leave", Map::default()).await
    }

    /// Sends an event on a topic
    pub async fn send(&mut self, topic: &str, event: &str, payload: Map) -> Result<Id, Error> {
        let id = self.msg_id.wrapping_add(1);

        let msg = ChannelMsg::<Cow<str>> {
            join_reference: None,
            message_reference: Cow::Owned(id.to_string()),
            topic_name: Cow::Borrowed(topic),
            event_name: Cow::Borrowed(event),
            payload,
        };
        let msg_json = serde_json::to_string(&msg).map_err(Error::Serialize)?;

        self.connection
            .send(Message::Text(msg_json.into()))
            .await
            .map_err(|err| Error::Send {
                msg: msg.into(),
                backtrace: err,
            })?;

        Ok(id)
    }
}
