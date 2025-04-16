//! Client for the Phoenix channel

use std::ops::DerefMut;
use std::pin::pin;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use async_tungstenite::tokio::ConnectStream;
use async_tungstenite::WebSocketStream;
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio::sync::Mutex;
use tracing::{debug, instrument, trace};
use tungstenite::http::Uri;

use crate::message::{ChannelMsg, Message};
use crate::{Builder, Error, Map};

/// Id to identify the response of a message sent by the client.
pub type Id = usize;

type Sender = SplitSink<WebSocketStream<ConnectStream>, tungstenite::Message>;
type Receiver = SplitStream<WebSocketStream<ConnectStream>>;

#[derive(Debug)]
struct Reader {
    heartbeat: tokio::time::Interval,
    receiver: Receiver,
}

/// Connection for the Phoenix channel
#[derive(Debug)]
pub struct Client {
    msg_id: AtomicUsize,
    sent: AtomicBool,
    writer: Mutex<Sender>,
    reader: Mutex<Reader>,
}

impl Client {
    pub(crate) fn new(connection: WebSocketStream<ConnectStream>, heartbeat: Duration) -> Self {
        let (writer, reader) = connection.split();
        Self {
            msg_id: AtomicUsize::new(0),
            sent: AtomicBool::new(false),
            writer: Mutex::new(writer),
            reader: Mutex::new(Reader {
                heartbeat: tokio::time::interval(heartbeat),
                receiver: reader,
            }),
        }
    }

    fn next_id(&self) -> usize {
        self.msg_id.fetch_add(1, Ordering::AcqRel)
    }

    /// Returns a builder to configure the client.
    pub fn builder(uri: Uri) -> Result<Builder, Error> {
        Builder::new(uri)
    }

    /// Joins a channel.
    pub async fn join(&self, topic: &str) -> Result<Id, Error> {
        self.join_with_payload(topic, Map::default()).await
    }

    /// Joins a channel with additional parameters.
    #[instrument(skip(self, payload))]
    pub async fn join_with_payload<P>(&self, topic: &str, payload: P) -> Result<Id, Error>
    where
        P: Serialize,
    {
        let id = self.next_id();

        let msg = ChannelMsg::new(Some(id), Some(id), topic, "phx_join", payload);

        debug!(id, "joining topic");

        self.write_msg(msg).await?;

        trace!(id, "topic joined");

        Ok(id)
    }

    /// Leaves a channel.
    #[instrument(skip(self))]
    pub async fn leave(&self, topic: &str) -> Result<Id, Error> {
        let id = self.next_id();

        let msg = ChannelMsg::new(None, Some(id), topic, "phx_leave", Map::default());

        debug!(id, "leaving topic");

        self.write_msg(msg).await?;

        trace!(id, "topic left");

        Ok(id)
    }

    /// Sends an event on a topic
    #[instrument(skip(self, payload))]
    pub async fn send<P>(&self, topic: &str, event: &str, payload: P) -> Result<Id, Error>
    where
        P: Serialize,
    {
        let id = self.next_id();

        let msg = ChannelMsg::new(None, Some(id), topic, event, payload);

        debug!(id, "sending event");

        self.write_msg(msg).await?;

        trace!(id, "event sent");

        Ok(id)
    }

    #[instrument(skip_all)]
    async fn write_msg<P>(&self, msg: ChannelMsg<'_, P>) -> Result<(), Error>
    where
        P: Serialize,
    {
        let msg_json = serde_json::to_string(&msg).map_err(Error::Serialize)?;

        trace!("writing on socket");

        self.writer
            .lock()
            .await
            .send(tungstenite::Message::Text(msg_json.into()))
            .await
            .map_err(Box::new)
            .map_err(|err| Error::Send {
                msg: msg.into_err(),
                backtrace: err,
            })?;

        trace!("update sent flag");

        self.sent.store(true, Ordering::Release);

        Ok(())
    }

    /// Returns the next message in any channel.
    #[instrument(skip(self))]
    pub async fn recv<P>(&self) -> Result<Message<P>, Error>
    where
        P: DeserializeOwned,
    {
        trace!("waiting for next message");

        let msg = self.next_msg().await?;

        trace!(%msg, "WebSocket message received");

        msg.into_text()
            .map_err(Box::new)
            .map_err(Error::WebSocketMessageType)
            .and_then(|txt| {
                serde_json::from_str::<ChannelMsg<P>>(txt.as_str()).map_err(Error::Deserialize)
            })
            .map(|msg| {
                let msg = Message::from(msg);

                debug!(message = msg.info(), "message received");

                msg
            })
    }

    #[instrument(skip(self))]
    async fn next_msg(&self) -> Result<tungstenite::Message, Error> {
        trace!("waiting for reader lock");
        let mut reader = self.reader.lock().await;
        let reader = reader.deref_mut();

        let mut receive = reader.receiver.next();

        loop {
            trace!("waiting for next event or heartbeat");
            match futures::future::select(pin!(reader.heartbeat.tick()), pin!(&mut receive)).await {
                futures::future::Either::Left((_instant, _next)) => {
                    trace!("heartbeat interval");
                    self.check_and_send_heartbeat().await?;
                }
                futures::future::Either::Right((None, _)) => {
                    debug!("WebSocket disconnected");

                    return Err(Error::Disconnected);
                }
                futures::future::Either::Right((Some(res), _)) => {
                    trace!("next event");

                    return res.map_err(Box::new).map_err(Error::Recv);
                }
            };
        }
    }

    #[instrument(skip(self))]
    async fn check_and_send_heartbeat(&self) -> Result<(), Error> {
        let val = self
            .sent
            .compare_exchange(true, false, Ordering::SeqCst, Ordering::Acquire);

        trace!(sent_flag = ?val, "heartbeat sent flag");

        match val {
            Ok(val) => {
                debug_assert!(val);
            }
            Err(val) => {
                debug_assert!(!val);

                let id = self.next_id();

                let heartbeat =
                    ChannelMsg::new(None, Some(id), "phoenix", "heartbeat", Map::default());

                debug!(id, "sending heartbeat");

                self.write_msg(heartbeat).await?;
            }
        }

        Ok(())
    }
}
