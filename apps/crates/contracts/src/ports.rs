use async_trait::async_trait;

use crate::error::ContractError;
use crate::message::Message;
use crate::model::WireMessage;
use crate::request::CompletionRequest;
use crate::transcript::TranscriptEntry;
use crate::usage::Usage;

/// 上下文投影、wire 转换与拥有型请求装配端口。
#[async_trait]
pub trait ContextEngine: Send + Sync {
    /// 从基础转录条目计算线性消息投影。
    fn project(&self, entries: &[TranscriptEntry]) -> Result<Vec<Message>, ContractError>;
    /// 在唯一领域到 wire 边界转换消息。
    fn to_wire(&self, messages: &[Message]) -> Result<Vec<WireMessage>, ContractError>;
    /// 构造可独立拥有的模型请求。
    async fn prepare_request(&self) -> Result<CompletionRequest, ContractError>;
}

/// 会话加载结果，保留坏行诊断而不把整体读取失败伪装成成功。
#[derive(Debug, Clone, PartialEq)]
pub struct SessionLoad {
    /// 成功读取的转录条目。
    pub entries: Vec<TranscriptEntry>,
    /// 被跳过条目等可行动诊断。
    pub diagnostics: Vec<ContractError>,
}

/// 只追加会话存储端口，不包含具体文件实现。
#[async_trait]
pub trait SessionRepository: Send + Sync {
    /// 追加一条转录条目。
    async fn append(&self, entry: TranscriptEntry) -> Result<(), ContractError>;
    /// 加载条目及坏行诊断；整体打开或读取失败仍返回 Err。
    async fn load(&self) -> Result<SessionLoad, ContractError>;
}

/// 独立用量台账记录端口。
#[async_trait]
pub trait UsageRecorder: Send + Sync {
    /// 记录一次用量。
    async fn record(&self, usage: Usage) -> Result<(), ContractError>;
}
