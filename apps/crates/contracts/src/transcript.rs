use serde::{Deserialize, Serialize};

use crate::message::Message;

/// 只追加转录中的基础消息条目。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "message", rename_all = "snake_case")]
pub enum TranscriptEntry {
    /// 一条完整领域消息，序列化为 `type = "message"` 与 `message` payload。
    Message(Message),
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::TranscriptEntry;
    use crate::message::{Content, Message};

    #[test]
    fn message_entry_has_exact_adjacently_tagged_json_and_roundtrips() {
        let entry = TranscriptEntry::Message(Message::user("你好"));

        let value = serde_json::to_value(&entry).unwrap();
        assert_eq!(
            value,
            serde_json::json!({
                "type": "message",
                "message": {
                    "role": "user",
                    "content": [{"type": "text", "text": "你好"}],
                    "uuid": "",
                    "parent_uuid": null
                }
            })
        );
        assert_eq!(
            serde_json::from_value::<TranscriptEntry>(value).unwrap(),
            entry
        );
    }

    #[test]
    fn message_entry_preserves_explicit_identity_and_meta_fields() {
        let mut message = Message::assistant(vec![Content::Text {
            text: "回复".into(),
        }]);
        message.is_meta = true;
        message.uuid = "m-2".into();
        message.parent_uuid = Some("m-1".into());
        let entry = TranscriptEntry::Message(message);

        let json = serde_json::to_string(&entry).unwrap();
        let restored = serde_json::from_str::<TranscriptEntry>(&json).unwrap();

        assert_eq!(restored, entry);
    }

    #[test]
    fn invalid_entry_tags_and_payloads_are_rejected() {
        let invalid = [
            r#"{"type":"unknown","message":{}}"#,
            r#"{"type":"message"}"#,
            r#"{"type":"message","message":"bad"}"#,
            r#"{"type":"message","message":{"content":[]}}"#,
            r#"{"type":"message","message":{"role":"bad","content":[]}}"#,
        ];

        for payload in invalid {
            assert!(
                serde_json::from_str::<TranscriptEntry>(payload).is_err(),
                "payload unexpectedly accepted: {payload}"
            );
        }
    }
}
