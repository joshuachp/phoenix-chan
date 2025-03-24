//! Client for the Phoenix channel

use std::borrow::Cow;
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
    pub fn builder(uri: Uri) -> Builder {
        Builder::new(uri)
    }

    /// Joins a channel.
    pub async fn join(&self, topic: &str) -> Result<Id, Error> {
        self.join_with_payload(topic, Map::default()).await
    }

    /// Joins a channel with additional parameters.
    pub async fn join_with_payload<P>(&self, topic: &str, payload: P) -> Result<Id, Error>
    where
        P: Serialize,
    {
        let id = self.next_id();

        let msg = ChannelMsg {
            join_reference: Some(Cow::Owned(id.to_string().into())),
            message_reference: Cow::Owned(id.to_string()),
            topic_name: Cow::Borrowed(topic),
            event_name: Cow::Borrowed("phx_join"),
            payload,
        };

        self.write_msg(msg).await?;

        Ok(id)
    }

    /// Leaves a channel.
    pub async fn leave(&self, topic: &str) -> Result<Id, Error> {
        self.send(topic, "phx_leave", Map::default()).await
    }

    /// Sends an event on a topic
    pub async fn send<P>(&self, topic: &str, event: &str, payload: P) -> Result<Id, Error>
    where
        P: Serialize,
    {
        let id = self.next_id();

        let msg = ChannelMsg {
            join_reference: None,
            message_reference: Cow::Owned(id.to_string()),
            topic_name: Cow::Borrowed(topic),
            event_name: Cow::Borrowed(event),
            payload,
        };

        self.write_msg(msg).await?;

        Ok(id)
    }

    async fn write_msg<P>(&self, msg: ChannelMsg<'_, P>) -> Result<(), Error>
    where
        P: Serialize,
    {
        let msg_json = serde_json::to_string(&msg).map_err(Error::Serialize)?;

        self.writer
            .lock()
            .await
            .send(tungstenite::Message::Text(msg_json.into()))
            .await
            .map_err(|err| Error::Send {
                msg: msg.into_err(),
                backtrace: err,
            })?;

        self.sent.store(true, Ordering::Release);

        Ok(())
    }

    /// Returns the next message in any channel.
    pub async fn recv<P>(&self) -> Result<Message<P>, Error>
    where
        P: DeserializeOwned,
    {
        let msg = self.next_msg().await?;

        msg.into_text()
            .map_err(Error::WebSocketMessageType)
            .and_then(|txt| {
                serde_json::from_str::<ChannelMsg<P>>(txt.as_str()).map_err(Error::Deserialize)
            })
            .map(Message::from)
    }

    async fn next_msg(&self) -> Result<tungstenite::Message, Error> {
        let mut reader = self.reader.lock().await;
        let reader = reader.deref_mut();

        let mut receive = reader.receiver.next();

        let next = loop {
            match futures::future::select(pin!(reader.heartbeat.tick()), pin!(&mut receive)).await {
                futures::future::Either::Left((_instant, _next)) => {
                    self.check_and_send_heartbeat().await?;
                }
                futures::future::Either::Right((next, _)) => break next,
            };
        };

        next.ok_or(Error::Disconnected)?.map_err(Error::Recv)
    }

    async fn check_and_send_heartbeat(&self) -> Result<(), Error> {
        let val = self
            .sent
            .compare_exchange(true, false, Ordering::SeqCst, Ordering::Acquire);

        match val {
            Ok(val) => {
                debug_assert!(val);
            }
            Err(val) => {
                debug_assert!(!val);

                let heartbeat = ChannelMsg {
                    join_reference: None,
                    message_reference: Cow::Owned(self.next_id().to_string()),
                    topic_name: Cow::Borrowed("phoenix"),
                    event_name: Cow::Borrowed("heartbeat"),
                    payload: Map::default(),
                };

                self.write_msg(heartbeat).await?;
            }
        }

        Ok(())
    }
}
