use serde::{Deserialize, Serialize};

/// 对话角色
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// 系统指令
    System,
    /// 用户
    User,
    /// 助手
    Assistant,
}

/// 消息内容块
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Content {
    /// 纯文本(`{"type": "text", "text": "hello"}`)
    Text { text: String },

    /// 模型发起的工具调用(`{"type": "tool_use", "id": "123", "name": "tool", "input": {"key": "value"}}}`)
    /// `id` 是模型调用的工具的唯一标识符，由模型自动生成
    /// `name` 是模型调用的工具的名称
    /// `input` 是模型调用的工具的输入参数
    /// 用于与回填的 `ToolResult` 配合使用。
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },

    /// 模型调用的工具的输出结果(`{"type": "tool_result", "tool_use_id": "123", "content": "hello", "is_error": false}`)
    /// `tool_use_id` 是模型调用的工具的唯一标识符，与 `ToolUse` 中的 `id` 一致
    /// `content` 是模型调用的工具的输出结果
    /// `is_error` 是模型调用的工具是否出错，为 `true` 时，`content` 为错误信息
    /// 用于与回填的 `ToolUse` 配合使用。
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },

    /// 思考(`{"type": "thinking", "text": "思考中..."}`)
    /// `text` 是模型的思考内容
    /// 用于与回填的 `Text` 配合使用。
    Thinking {
        thinking: String,
        signature: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Message {
    /// 对话角色（system、user 或 assistant）
    pub role: Role,

    /// 消息内容块
    pub content: Vec<Content>,

    /// 是否为元消息（AGENT.md 等）
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_meta: bool,

    /// 会话树节点id: Session 适配器 append 时赋值
    #[serde(default)]
    pub uuid: String,

    /// 父节点uuid: Session 适配器 append 时赋值
    #[serde(default)]
    pub parent_uuid: Option<String>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

impl Message {
    /// system 文本消息。
    pub fn system(text: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: vec![Content::Text { text: text.into() }],
            is_meta: false,
            uuid: String::new(),
            parent_uuid: None,
        }
    }

    /// 普通 user 文本消息
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: vec![Content::Text { text: text.into() }],
            is_meta: false,
            uuid: String::new(),
            parent_uuid: None,
        }
    }

    /// isMeta 注入的上下文消息（AGENT.md 等），以 user 角色「伪装」进对话。
    pub fn user_meta(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: vec![Content::Text { text: text.into() }],
            is_meta: true,
            uuid: String::new(),
            parent_uuid: None,
        }
    }

    /// assistant 消息：内容块由调用方组装（text / tool_use / thinking 任意组合）。
    pub fn assistant(content: Vec<Content>) -> Self {
        Self {
            role: Role::Assistant,
            content,
            is_meta: false,
            uuid: String::new(),
            parent_uuid: None,
        }
    }

    /// tool_result 在协议上作为 user 角色消息回填（与 OpenAI tool 角色映射由 provider 处理）。
    pub fn tool_results(results: Vec<Content>) -> Self {
        Self {
            role: Role::User,
            content: results,
            is_meta: false,
            uuid: String::new(),
            parent_uuid: None,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn message_serde_roundtrip() {
        let msgs = vec![
            Message::user("你好"),
            Message::assistant(vec![
                Content::Text {
                    text: "看一下".into(),
                },
                Content::ToolUse {
                    id: "call_1".into(),
                    name: "Read".into(),
                    input: serde_json::json!({"path": "/tmp/a.txt"}),
                },
            ]),
            Message::tool_results(vec![Content::ToolResult {
                tool_use_id: "call_1".into(),
                content: "文件内容".into(),
                is_error: false,
            }]),
        ];
        let json = serde_json::to_string(&msgs).unwrap();
        let back: Vec<Message> = serde_json::from_str(&json).unwrap();
        assert_eq!(msgs, back);
    }

    #[test]
    fn is_meta_defaults_false_for_old_transcript_lines() {
        let old = r#"{"role":"user","content":[{"type":"text","text":"hi"}]}"#;
        let m: Message = serde_json::from_str(old).unwrap();
        assert!(!m.is_meta);
        assert_eq!(m, Message::user("hi"));
    }

    #[test]
    fn message_uuid_fields_serde_default_compatible() {
        // 旧格式 transcript 行（无 uuid/parent_uuid 字段）反序列化为默认值
        let old = r#"{"role":"user","content":[{"type":"text","text":"hi"}]}"#;
        let m: Message = serde_json::from_str(old).unwrap();
        assert_eq!(m.uuid, "");
        assert_eq!(m.parent_uuid, None);
        assert!(!m.is_meta);
        // roundtrip：新格式带字段
        let mut m2 = Message::user("x");
        m2.uuid = "msg-1-0".into();
        m2.parent_uuid = Some("msg-0-0".into());
        let s = serde_json::to_string(&m2).unwrap();
        let back: Message = serde_json::from_str(&s).unwrap();
        assert_eq!(back, m2);
    }

    #[test]
    fn is_meta_false_omitted_in_serialization() {
        // 恒 false 字段不落盘
        let s = serde_json::to_string(&Message::user("hi")).unwrap();
        assert!(!s.contains("is_meta"), "false 不序列化: {s}");
        let s2 = serde_json::to_string(&Message::user_meta("ctx")).unwrap();
        assert!(s2.contains("\"is_meta\":true"), "true 仍序列化: {s2}");
        // 旧 transcript 行（含显式 false 或无字段）均可读
        let old_explicit =
            r#"{"role":"user","content":[{"type":"text","text":"a"}],"is_meta":false}"#;
        let m: Message = serde_json::from_str(old_explicit).unwrap();
        assert!(!m.is_meta);
    }

    #[test]
    fn user_meta_constructor_sets_flag() {
        let m = Message::user_meta("<system-reminder>ctx</system-reminder>");
        assert!(m.is_meta);
        assert_eq!(m.role, Role::User);
        let s = serde_json::to_string(&m).unwrap();
        let back: Message = serde_json::from_str(&s).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn content_thinking_serde_roundtrip() {
        // Anthropic thinking 块——signature 为多轮回放必需凭证
        let b = Content::Thinking {
            thinking: "先分析边界".into(),
            signature: Some("sig_abc".into()),
        };
        let v = serde_json::to_value(&b).unwrap();
        assert_eq!(
            v,
            serde_json::json!({"type": "thinking", "thinking": "先分析边界", "signature": "sig_abc"})
        );
        let back: Content = serde_json::from_value(v).unwrap();
        assert_eq!(back, b);
        // signature=None（跨协议降级形态）同样 roundtrip
        let b2 = Content::Thinking {
            thinking: "t".into(),
            signature: None,
        };
        let s = serde_json::to_string(&b2).unwrap();
        let back2: Content = serde_json::from_str(&s).unwrap();
        assert_eq!(back2, b2);
    }

    #[test]
    fn content_serde_tags() {
        let b = Content::Text { text: "x".into() };
        assert_eq!(
            serde_json::to_value(&b).unwrap(),
            serde_json::json!({"type": "text", "text": "x"})
        );
        let b = Content::ToolResult {
            tool_use_id: "c".into(),
            content: "ok".into(),
            is_error: true,
        };
        assert_eq!(
            serde_json::to_value(&b).unwrap(),
            serde_json::json!({"type": "tool_result", "tool_use_id": "c", "content": "ok", "is_error": true})
        );
    }

    #[test]
    fn system_role_uses_stable_json_and_existing_roles_remain_readable() {
        assert_eq!(serde_json::to_value(Role::System).unwrap(), "system");
        assert_eq!(
            serde_json::from_str::<Role>(r#""system""#).unwrap(),
            Role::System
        );
        assert_eq!(
            serde_json::from_str::<Role>(r#""user""#).unwrap(),
            Role::User
        );
        assert_eq!(
            serde_json::from_str::<Role>(r#""assistant""#).unwrap(),
            Role::Assistant
        );
    }

    #[test]
    fn system_constructor_sets_role_without_meta_or_identity_defaults() {
        let message = Message::system("规则");

        assert_eq!(message.role, Role::System);
        assert_eq!(
            message.content,
            vec![Content::Text {
                text: "规则".into()
            }]
        );
        assert!(!message.is_meta);
        assert_eq!(message.uuid, "");
        assert_eq!(message.parent_uuid, None);
    }

    #[test]
    fn invalid_message_payloads_are_rejected() {
        let invalid = [
            r#"{"content":[]}"#,
            r#"{"role":"user"}"#,
            r#"{"role":"unknown","content":[]}"#,
            r#"{"role":"user","content":"text"}"#,
            r#"{"role":"user","content":[],"uuid":7}"#,
            r#"{"role":"user","content":[],"parent_uuid":false}"#,
        ];

        for payload in invalid {
            assert!(
                serde_json::from_str::<Message>(payload).is_err(),
                "payload unexpectedly accepted: {payload}"
            );
        }
    }

    #[test]
    fn explicit_non_default_message_fields_have_exact_json() {
        let mut message = Message::user_meta("上下文");
        message.uuid = "m-2".into();
        message.parent_uuid = Some("m-1".into());

        assert_eq!(
            serde_json::to_value(&message).unwrap(),
            serde_json::json!({
                "role": "user",
                "content": [{"type": "text", "text": "上下文"}],
                "is_meta": true,
                "uuid": "m-2",
                "parent_uuid": "m-1"
            })
        );
    }
}
