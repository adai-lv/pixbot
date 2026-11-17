use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::usage::Usage;

/// 工具返回给模型的内容块，与领域消息内容类型相互独立。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolContent {
    /// 文本结果，允许空字符串与 Unicode。
    Text { text: String },
}

/// 工具执行结果的唯一失败标记与三通道内容。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolResult {
    /// 发给模型的内容块。
    pub content: Vec<ToolContent>,
    /// 仅用于前端或日志的细节，不发给模型。
    #[serde(default)]
    pub details: Option<serde_json::Value>,
    /// 由输出 schema 描述的结构化结果，不发给模型。
    #[serde(default)]
    pub structured_content: Option<serde_json::Value>,
    /// 工具自身用量，不并入主上下文估算。
    #[serde(default)]
    pub usage: Option<Usage>,
    /// 工具是否失败；没有默认值，避免失败被静默解释为成功。
    pub is_error: bool,
    /// 是否请求提前终止；缺省为 false。
    #[serde(default)]
    pub terminate: bool,
}

/// 工具声明，供请求装配和 Provider 映射使用。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolDefinition {
    /// 稳定的工具名。
    pub name: String,
    /// 展示给模型的工具说明。
    pub description: String,
    /// JSON Schema 输入约束。
    pub input_schema: serde_json::Value,
    /// 可选 JSON Schema 输出约束。
    #[serde(default)]
    pub output_schema: Option<serde_json::Value>,
}

/// 工具批次的执行方式。
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolExecutionMode {
    /// 默认并行执行。
    #[default]
    Parallel,
    /// 要求批次顺序执行。
    Sequential,
}

/// 一次工具调用的上下文。
#[derive(Clone)]
pub struct ToolContext {
    /// 与模型工具调用配对的标识。
    pub call_id: String,
    /// 工具执行的工作目录。
    pub cwd: PathBuf,
    /// 协作式取消令牌。
    pub cancel: CancellationToken,
    /// 完整部分结果回调；同步调用，不承诺后台调度。
    pub on_update: Option<Arc<dyn Fn(ToolResult) + Send + Sync>>,
}

use async_trait::async_trait;

use crate::error::ContractError;

/// 工具定义与执行端口，不包含 hooks、replay 或 constrained sampling。
#[async_trait]
pub trait Tool: Send + Sync {
    /// 稳定工具名。
    fn name(&self) -> &str;
    /// 工具说明。
    fn description(&self) -> &str;
    /// 输入 JSON Schema。
    fn input_schema(&self) -> &serde_json::Value;
    /// 可选输出 JSON Schema。
    fn output_schema(&self) -> Option<&serde_json::Value> {
        None
    }
    /// 在执行前预备参数；默认保持输入不变。
    fn prepare_arguments(
        &self,
        input: serde_json::Value,
    ) -> Result<serde_json::Value, ContractError> {
        Ok(input)
    }
    /// 执行工具并返回完整结果或中立错误。
    async fn execute(
        &self,
        context: ToolContext,
        input: serde_json::Value,
    ) -> Result<ToolResult, ContractError>;
    /// 声明批次执行方式，默认并行。
    fn execution_mode(&self) -> ToolExecutionMode {
        ToolExecutionMode::Parallel
    }
}

/// 按稳定注册序声明并查找工具的端口。
pub trait ToolSet: Send + Sync {
    /// 返回当前活动工具声明。
    fn definitions(&self) -> Vec<ToolDefinition>;
    /// 按名称查找工具；未知工具返回 None。
    fn get(&self, name: &str) -> Option<Arc<dyn Tool>>;
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    use super::{ToolContent, ToolContext, ToolDefinition, ToolExecutionMode, ToolResult};
    use crate::usage::Usage;

    fn result(text: &str, is_error: bool) -> ToolResult {
        ToolResult {
            content: vec![ToolContent::Text { text: text.into() }],
            details: None,
            structured_content: None,
            usage: None,
            is_error,
            terminate: false,
        }
    }

    #[test]
    fn tool_content_preserves_empty_and_unicode_text() {
        for text in ["", "你好 🦀"] {
            let content = ToolContent::Text { text: text.into() };
            let value = serde_json::to_value(&content).unwrap();

            assert_eq!(value, serde_json::json!({"type": "text", "text": text}));
            assert_eq!(
                serde_json::from_value::<ToolContent>(value).unwrap(),
                content
            );
        }
    }

    #[test]
    fn tool_result_defaults_optional_channels_and_terminate() {
        let result =
            serde_json::from_str::<ToolResult>(r#"{"content":[],"is_error":true}"#).unwrap();

        assert!(result.content.is_empty());
        assert_eq!(result.details, None);
        assert_eq!(result.structured_content, None);
        assert_eq!(result.usage, None);
        assert!(result.is_error);
        assert!(!result.terminate);
    }

    #[test]
    fn tool_result_requires_explicit_error_status() {
        assert!(serde_json::from_str::<ToolResult>(r#"{"content":[]}"#).is_err());
    }

    #[test]
    fn tool_result_preserves_all_channels_and_zero_usage() {
        let result = ToolResult {
            usage: Some(Usage::default()),
            details: Some(serde_json::json!({"line": 0})),
            structured_content: Some(serde_json::json!({"ok": false})),
            terminate: true,
            ..result("失败", true)
        };

        let value = serde_json::to_value(&result).unwrap();
        assert_eq!(value["content"][0]["text"], "失败");
        assert_eq!(value["details"]["line"], 0);
        assert_eq!(value["structured_content"]["ok"], false);
        assert_eq!(value["usage"]["total_tokens"], 0);
        assert_eq!(value["is_error"], true);
        assert_eq!(value["terminate"], true);
        assert_eq!(serde_json::from_value::<ToolResult>(value).unwrap(), result);
    }

    #[test]
    fn tool_definition_supports_input_and_optional_output_schema() {
        let definition = ToolDefinition {
            name: "read".into(),
            description: "读取文本".into(),
            input_schema: serde_json::json!({"type": "object"}),
            output_schema: None,
        };

        let value = serde_json::to_value(&definition).unwrap();
        assert_eq!(value["name"], "read");
        assert_eq!(value["output_schema"], serde_json::Value::Null);
        assert_eq!(
            serde_json::from_value::<ToolDefinition>(value).unwrap(),
            definition
        );
        assert!(
            serde_json::from_str::<ToolDefinition>(
                r#"{"name":"read","description":"读取文本","input_schema":{}}"#
            )
            .is_ok()
        );
    }

    #[test]
    fn tool_execution_mode_has_only_parallel_and_sequential_json() {
        assert_eq!(
            serde_json::to_value(ToolExecutionMode::Parallel).unwrap(),
            "parallel"
        );
        assert_eq!(
            serde_json::to_value(ToolExecutionMode::Sequential).unwrap(),
            "sequential"
        );
        assert_eq!(ToolExecutionMode::default(), ToolExecutionMode::Parallel);
        assert!(serde_json::from_str::<ToolExecutionMode>(r#""replay""#).is_err());
    }

    #[test]
    fn tool_context_carries_identity_cancellation_and_updates() {
        let updates = Arc::new(Mutex::new(Vec::new()));
        let captured = Arc::clone(&updates);
        let context = ToolContext {
            call_id: "call-1".into(),
            cwd: PathBuf::from("/work"),
            cancel: tokio_util::sync::CancellationToken::new(),
            on_update: Some(Arc::new(move |result| {
                captured
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(result);
            })),
        };

        context.cancel.cancel();
        let update = context.on_update.as_ref().unwrap();
        update(result("部分", false));

        assert_eq!(context.call_id, "call-1");
        assert_eq!(context.cwd, PathBuf::from("/work"));
        assert!(context.cancel.is_cancelled());
        let updates = updates.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(updates.len(), 1);
        assert!(!updates[0].is_error);
    }
}
