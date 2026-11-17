use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// 中立错误的归属领域。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorDomain {
    /// Agent 控制内核。
    Agent,
    /// 上下文投影与请求装配。
    Context,
    /// 模型 Provider。
    Provider,
    /// 工具声明或执行。
    Tool,
    /// 会话持久化。
    Session,
    /// 用量台账。
    Usage,
    /// 配置加载或快照。
    Config,
}

impl fmt::Display for ErrorDomain {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Agent => "agent",
            Self::Context => "context",
            Self::Provider => "provider",
            Self::Tool => "tool",
            Self::Session => "session",
            Self::Usage => "usage",
            Self::Config => "config",
        };
        formatter.write_str(name)
    }
}

/// 所有跨 crate 端口共享的可行动错误分类。
#[derive(Debug, Clone, Error, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ContractError {
    /// 当前运行不可重入。
    #[error("{domain}: {message}")]
    Busy {
        domain: ErrorDomain,
        message: String,
    },
    /// 输入参数或状态不满足端口约束。
    #[error("{domain}: {message}")]
    InvalidInput {
        domain: ErrorDomain,
        message: String,
    },
    /// 请求的资源不存在。
    #[error("{domain}: {message}")]
    NotFound {
        domain: ErrorDomain,
        message: String,
    },
    /// 请求的能力不受支持。
    #[error("{domain}: {message}")]
    Unsupported {
        domain: ErrorDomain,
        message: String,
    },
    /// 调用被协作取消，永不自动重试。
    #[error("{domain}: {message}")]
    Aborted {
        domain: ErrorDomain,
        message: String,
    },
    /// 认证或凭据无效。
    #[error("{domain}: {message}")]
    Authentication {
        domain: ErrorDomain,
        message: String,
    },
    /// 连接失败，由调用方记录是否可重试。
    #[error("{domain}: {message}")]
    Connection {
        domain: ErrorDomain,
        message: String,
        retryable: bool,
    },
    /// 操作超时，由调用方记录是否可重试。
    #[error("{domain}: {message}")]
    Timeout {
        domain: ErrorDomain,
        message: String,
        retryable: bool,
    },
    /// 请求被限流，与配额耗尽分型。
    #[error("{domain}: {message}")]
    RateLimited {
        domain: ErrorDomain,
        message: String,
        retryable: bool,
    },
    /// 配额耗尽，不表示可以原样重试。
    #[error("{domain}: {message}")]
    QuotaExceeded {
        domain: ErrorDomain,
        message: String,
    },
    /// 中立协议或远端响应不符合契约。
    #[error("{domain}: {message}")]
    Protocol {
        domain: ErrorDomain,
        message: String,
    },
    /// 上下文超过预算，不表示原请求可直接重试。
    #[error("{domain}: {message}")]
    ContextOverflow {
        domain: ErrorDomain,
        message: String,
    },
    /// 存储打开、读取、写入或刷新失败。
    #[error("{domain}: {message}")]
    Storage {
        domain: ErrorDomain,
        message: String,
    },
    /// 不属于其他分类的内部失败。
    #[error("{domain}: {message}")]
    Internal {
        domain: ErrorDomain,
        message: String,
    },
}

impl ContractError {
    /// 只有显式携带重试决策的连接、超时和限流错误可以返回 true。
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Connection { retryable, .. }
            | Self::Timeout { retryable, .. }
            | Self::RateLimited { retryable, .. } => *retryable,
            _ => false,
        }
    }

    /// 是否表示协作取消。
    pub fn is_abort(&self) -> bool {
        matches!(self, Self::Aborted { .. })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::{ContractError, ErrorDomain};

    #[test]
    fn retryable_categories_keep_their_explicit_decision() {
        let connection = ContractError::Connection {
            domain: ErrorDomain::Provider,
            message: "断线".into(),
            retryable: true,
        };
        let timeout = ContractError::Timeout {
            domain: ErrorDomain::Provider,
            message: "超时".into(),
            retryable: false,
        };
        let rate_limited = ContractError::RateLimited {
            domain: ErrorDomain::Provider,
            message: "限流".into(),
            retryable: true,
        };

        assert!(connection.is_retryable());
        assert!(!timeout.is_retryable());
        assert!(rate_limited.is_retryable());
        assert!(!connection.is_abort());
    }

    #[test]
    fn abort_quota_and_context_overflow_have_distinct_semantics() {
        let aborted = ContractError::Aborted {
            domain: ErrorDomain::Agent,
            message: "已取消".into(),
        };
        let quota = ContractError::QuotaExceeded {
            domain: ErrorDomain::Provider,
            message: "配额耗尽".into(),
        };
        let overflow = ContractError::ContextOverflow {
            domain: ErrorDomain::Context,
            message: "上下文过长".into(),
        };

        assert!(aborted.is_abort());
        assert!(!aborted.is_retryable());
        assert!(!quota.is_abort());
        assert!(!quota.is_retryable());
        assert!(!overflow.is_abort());
        assert!(!overflow.is_retryable());
    }

    #[test]
    fn every_non_retryable_variant_preserves_domain_and_owned_message() {
        let errors = vec![
            ContractError::Busy {
                domain: ErrorDomain::Agent,
                message: "busy".into(),
            },
            ContractError::InvalidInput {
                domain: ErrorDomain::Tool,
                message: "invalid".into(),
            },
            ContractError::NotFound {
                domain: ErrorDomain::Session,
                message: "missing".into(),
            },
            ContractError::Unsupported {
                domain: ErrorDomain::Provider,
                message: "unsupported".into(),
            },
            ContractError::Authentication {
                domain: ErrorDomain::Provider,
                message: "auth".into(),
            },
            ContractError::Protocol {
                domain: ErrorDomain::Provider,
                message: "protocol".into(),
            },
            ContractError::Storage {
                domain: ErrorDomain::Session,
                message: "storage".into(),
            },
            ContractError::Internal {
                domain: ErrorDomain::Usage,
                message: "internal".into(),
            },
        ];

        for error in errors {
            assert!(!error.is_retryable());
            assert!(!error.is_abort());
            let value = serde_json::to_value(&error).unwrap();
            assert!(value.to_string().contains("message"));
            let restored = serde_json::from_value::<ContractError>(value).unwrap();
            assert_eq!(restored, error);
        }
    }

    #[test]
    fn error_domain_covers_each_neutral_owner() {
        let domains = [
            ErrorDomain::Agent,
            ErrorDomain::Context,
            ErrorDomain::Provider,
            ErrorDomain::Tool,
            ErrorDomain::Session,
            ErrorDomain::Usage,
            ErrorDomain::Config,
        ];

        for domain in domains {
            let error = ContractError::InvalidInput {
                domain,
                message: "输入错误".into(),
            };
            let display = error.to_string();
            assert!(display.contains("输入错误"));
            let json = serde_json::to_string(&error).unwrap();
            assert_eq!(serde_json::from_str::<ContractError>(&json).unwrap(), error);
        }
    }
}
