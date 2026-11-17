use serde::{Deserialize, Serialize};

use crate::model::{ModelMetadata, WireMessage};
use crate::tool::ToolDefinition;

/// 一次模型调用的拥有型中立请求。
///
/// 请求不携带认证、连接、重试或厂商扩展，也不借用上下文投影。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompletionRequest {
    /// 本次运行前已经选定的模型元数据。
    pub model: ModelMetadata,
    /// 已完成领域到 wire 转换的消息。
    pub messages: Vec<WireMessage>,
    /// 本次请求暴露的工具声明。
    pub tools: Vec<ToolDefinition>,
}
