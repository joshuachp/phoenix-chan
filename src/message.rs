//! Messages sent from and to the phoenix channel.

use std::borrow::Cow;
use std::fmt::{Debug, Display};
use std::marker::PhantomData;

use serde::de::Visitor;
use serde::ser::SerializeSeq;
use serde::{Deserialize, Serialize};

use crate::client::Id;

/// Message received from the channel.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Message<P> {
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
    pub message_reference: Option<String>,
    /// The `topic_name` must be a known topic for the socket endpoint, and a client must join that
    /// topic before sending any messages on it.
    pub topic_name: String,
    /// The `event_name` must match the first argument of a `handle_in` function on the server channel
    /// module.
    pub event_name: String,
    /// The `payload` should be a map and is passed as the second argument to that `handle_in`
    /// function.
    pub payload: P,
}

impl<P> Message<P> {
    pub(crate) fn info(&self) -> String {
        format!(
            "[{:?}, {:?}, {:?}, {:?}, <payload>]",
            self.join_reference, self.message_reference, self.topic_name, self.event_name
        )
    }
}

impl<'a, P> From<ChannelMsg<'a, P>> for Message<P> {
    fn from(value: ChannelMsg<'a, P>) -> Self {
        Self {
            join_reference: value.join_reference.map(Cow::into),
            message_reference: value.message_reference.map(Cow::into),
            topic_name: value.topic_name.into(),
            event_name: value.event_name.into(),
            payload: value.payload,
        }
    }
}

impl Message<serde_json::Value> {
    /// Deserialize the value in a specific payload type.
    ///
    /// This makes it possible to match on the [`topic_name`](Message::topic_name) and
    /// [`event_name`](Message::event_name) to differentiate the various responses.
    pub fn deserialize_payload<P>(self) -> Result<Self, serde_json::error::Error> {
        let payload = serde_json::from_value(self.payload)?;

        Ok(Self {
            join_reference: self.join_reference,
            message_reference: self.message_reference,
            topic_name: self.topic_name,
            event_name: self.event_name,
            payload,
        })
    }
}

impl<P> Display for Message<P>
where
    P: Serialize + Debug,
{
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ChannelMsg<'a, P> {
    pub(crate) join_reference: Option<Cow<'a, str>>,
    pub(crate) message_reference: Option<Cow<'a, str>>,
    pub(crate) topic_name: Cow<'a, str>,
    pub(crate) event_name: Cow<'a, str>,
    pub(crate) payload: P,
}

impl<'a, P> ChannelMsg<'a, P> {
    pub(crate) fn new(
        join_reference: Option<Id>,
        message_reference: Option<Id>,
        topic_name: &'a str,
        event_name: &'a str,
        payload: P,
    ) -> Self {
        Self {
            join_reference: join_reference.map(|id| Cow::Owned(id.to_string())),
            message_reference: message_reference.map(|id| Cow::Owned(id.to_string())),
            topic_name: Cow::Borrowed(topic_name),
            event_name: Cow::Borrowed(event_name),
            payload,
        }
    }

    pub(crate) fn into_err(self) -> Message<()> {
        Message {
            join_reference: self.join_reference.map(Cow::into),
            message_reference: self.message_reference.map(Cow::into),
            topic_name: self.topic_name.into(),
            event_name: self.event_name.into(),
            payload: (),
        }
    }
}

impl<P> Serialize for ChannelMsg<'_, P>
where
    P: Serialize,
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

impl<'de, 'a, P> Deserialize<'de> for ChannelMsg<'a, P>
where
    P: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        #[derive(Debug)]
        struct ChannelMsgVisitor<'a, P> {
            _marker: PhantomData<(Cow<'a, str>, P)>,
        }

        impl<'de, 'a, P> Visitor<'de> for ChannelMsgVisitor<'a, P>
        where
            P: Deserialize<'de>,
        {
            type Value = ChannelMsg<'a, P>;

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

                Ok(ChannelMsg::<P> {
                    join_reference,
                    message_reference,
                    topic_name,
                    event_name,
                    payload,
                })
            }
        }

        deserializer.deserialize_seq(ChannelMsgVisitor::<'a, P> {
            _marker: PhantomData,
        })
    }
}

impl<P> Display for ChannelMsg<'_, P>
where
    P: Serialize + Debug,
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

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use crate::Map;

    use super::*;

    #[test]
    fn serialize_deserialize_join() {
        let join = r#"["0","0","miami:weather","phx_join",{"some":"param"}]"#;

        let message: ChannelMsg<Map> = serde_json::from_str(join).unwrap();

        let exp = ChannelMsg::new(
            Some(0),
            Some(0),
            "miami:weather",
            "phx_join",
            Map::from_iter([("some".to_string(), "param".to_string())]),
        );

        assert_eq!(message, exp);

        let json = serde_json::to_string(&message).unwrap();

        assert_eq!(json, join);
    }

    #[test]
    fn serialize_deserialize_leave() {
        let join = r#"[null,"1","miami:weather","phx_leave",{}]"#;

        let message: ChannelMsg<Map> = serde_json::from_str(join).unwrap();

        let exp = ChannelMsg::new(None, Some(1), "miami:weather", "phx_leave", Map::default());

        assert_eq!(message, exp);

        let json = serde_json::to_string(&message).unwrap();

        assert_eq!(json, join);
    }

    #[test]
    fn serialize_deserialize_heartbit() {
        let join = r#"[null,"2","phoenix","heartbeat",{}]"#;

        let message: ChannelMsg<Map> = serde_json::from_str(join).unwrap();

        let exp = ChannelMsg::new(None, Some(2), "phoenix", "heartbeat", Map::default());

        assert_eq!(message, exp);

        let json = serde_json::to_string(&message).unwrap();

        assert_eq!(json, join);
    }

    #[test]
    fn serialize_deserialize_send_example() {
        let join = r#"[null,"3","miami:weather","report_emergency",{"category":"sharknado"}]"#;

        let message: ChannelMsg<Map> = serde_json::from_str(join).unwrap();

        let exp = ChannelMsg::new(
            None,
            Some(3),
            "miami:weather",
            "report_emergency",
            Map::from_iter([("category".to_string(), "sharknado".to_string())]),
        );

        assert_eq!(message, exp);

        let json = serde_json::to_string(&message).unwrap();

        assert_eq!(json, join);
    }

    #[test]
    fn serialize_deserialize_no_message_reference() {
        let join =
            r#"[null,null,"rooms:test:dashboard_oSgokqqBReiKRg_c1nYqMQ_9899","watch_added",{}]"#;

        let message: ChannelMsg<Map> = serde_json::from_str(join).unwrap();

        let exp = ChannelMsg::new(
            None,
            None,
            "rooms:test:dashboard_oSgokqqBReiKRg_c1nYqMQ_9899",
            "watch_added",
            Map::default(),
        );

        assert_eq!(message, exp);

        let json = serde_json::to_string(&message).unwrap();

        assert_eq!(json, join);
    }
}
