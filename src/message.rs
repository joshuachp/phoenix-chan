//! Messages sent from and to the phoenix channel.

use std::fmt::{Debug, Display};
use std::marker::PhantomData;

use serde::de::Visitor;
use serde::ser::SerializeSeq;
use serde::{Deserialize, Serialize};

use crate::Map;

/// Message received from the channel.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Message {
    /// The `join_reference` is also chosen by the client and should also be a unique value.
    ///
    /// It only needs to be sent for a `phx_join` event; for other messages it can be null. It is
    /// used as a message reference for push messages from the server, meaning those that are not
    /// replies to a specific client message. For example, imagine something like "a new user just
    /// joined the chat room".
    pub join_reference: Option<String>,
    /// The `message_reference` is chosen by the client and should be a unique value.
    ///
    /// The server includes it in its reply so that the client knows which message the reply is for.
    pub message_reference: String,
    /// The `topic_name` must be a known topic for the socket endpoint, and a client must join that
    /// topic before sending any messages on it.
    pub topic_name: String,
    /// The `event_name` must match the first argument of a `handle_in` function on the server channel
    /// module.
    pub event_name: String,
    /// The `payload` should be a map and is passed as the second argument to that `handle_in`
    /// function.
    pub payload: Map,
}

impl<S> From<ChannelMsg<S>> for Message
where
    S: Into<String>,
{
    fn from(value: ChannelMsg<S>) -> Self {
        Self {
            join_reference: value.join_reference.map(S::into),
            message_reference: value.message_reference.into(),
            topic_name: value.topic_name.into(),
            event_name: value.event_name.into(),
            payload: value.payload,
        }
    }
}

impl Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[")?;
        ser_or_debug(&self.join_reference, f)?;
        write!(f, ", ")?;
        ser_or_debug(&self.message_reference, f)?;
        write!(f, ", ")?;
        ser_or_debug(&self.topic_name, f)?;
        write!(f, ", ")?;
        ser_or_debug(&self.event_name, f)?;
        write!(f, ", ")?;
        ser_or_debug(&self.payload, f)?;
        write!(f, "]")
    }
}

fn ser_or_debug<T>(v: &T, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result
where
    T: Serialize + Debug,
{
    if let Ok(s) = serde_json::to_string(v) {
        write!(f, "{s}")
    } else {
        write!(f, "{v:?}")
    }
}

#[derive(Debug)]
pub(crate) struct ChannelMsg<S = String> {
    pub(crate) join_reference: Option<S>,
    pub(crate) message_reference: S,
    pub(crate) topic_name: S,
    pub(crate) event_name: S,
    pub(crate) payload: Map,
}

impl<T> Serialize for ChannelMsg<T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut s = serializer.serialize_seq(Some(5))?;
        s.serialize_element(&self.join_reference)?;
        s.serialize_element(&self.message_reference)?;
        s.serialize_element(&self.topic_name)?;
        s.serialize_element(&self.event_name)?;
        s.serialize_element(&self.payload)?;
        s.end()
    }
}

impl<'de, T> Deserialize<'de> for ChannelMsg<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        #[derive(Debug)]
        struct ChannelMsgVisitor<T> {
            _marker: PhantomData<T>,
        }

        impl<'de, T> Visitor<'de> for ChannelMsgVisitor<T>
        where
            T: Deserialize<'de>,
        {
            type Value = ChannelMsg<T>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(
                    formatter,
                    "a sequence of 5 elements for a valid Phoenix channel"
                )
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                if let Some(len) = seq.size_hint() {
                    if len != 5 {
                        return Err(A::Error::invalid_length(len, &"5"));
                    }
                }

                let Some(join_reference) = seq.next_element()? else {
                    return Err(A::Error::invalid_length(0, &"5"));
                };
                let Some(message_reference) = seq.next_element()? else {
                    return Err(A::Error::invalid_length(1, &"5"));
                };
                let Some(topic_name) = seq.next_element()? else {
                    return Err(A::Error::invalid_length(2, &"5"));
                };
                let Some(event_name) = seq.next_element()? else {
                    return Err(A::Error::invalid_length(3, &"5"));
                };
                let Some(payload) = seq.next_element()? else {
                    return Err(A::Error::invalid_length(4, &"5"));
                };

                Ok(ChannelMsg::<T> {
                    join_reference,
                    message_reference,
                    topic_name,
                    event_name,
                    payload,
                })
            }
        }

        deserializer.deserialize_seq(ChannelMsgVisitor::<T> {
            _marker: PhantomData,
        })
    }
}

impl<S> Display for ChannelMsg<S>
where
    S: Serialize + Debug + Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Ok(s) = serde_json::to_string(self) else {
            return write!(
                f,
                "[{:?}, {:?}, {:?}, {:?}, {:?}]",
                self.join_reference,
                self.message_reference,
                self.topic_name,
                self.event_name,
                self.payload,
            );
        };

        write!(f, "{s}")
    }
}
