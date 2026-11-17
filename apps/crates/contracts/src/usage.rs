use serde::{Deserialize, Serialize};

/// 模型或工具的一次用量计量；字段缺省时按零计量。
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Usage {
    /// 输入 token 数。
    pub input: u64,
    /// 输出 token 数。
    pub output: u64,
    /// 缓存读取 token 数。
    pub cache_read: u64,
    /// 缓存写入 token 数。
    pub cache_write: u64,
    /// 一小时缓存写入 token 数。
    pub cache_write_1h: u64,
    /// 推理 token 数，是输出的子集。
    pub reasoning: u64,
    /// 总 token 数。
    pub total_tokens: u64,
    /// 上报成本；本契约只透传计量，不执行计费。
    pub cost: f64,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::Usage;

    #[test]
    fn missing_usage_fields_default_to_zero() {
        let usage = serde_json::from_str::<Usage>("{}").unwrap();

        assert_eq!(usage, Usage::default());
        assert_eq!(usage.input, 0);
        assert_eq!(usage.output, 0);
        assert_eq!(usage.cache_read, 0);
        assert_eq!(usage.cache_write, 0);
        assert_eq!(usage.cache_write_1h, 0);
        assert_eq!(usage.reasoning, 0);
        assert_eq!(usage.total_tokens, 0);
        assert_eq!(usage.cost, 0.0);
    }

    #[test]
    fn usage_preserves_all_eight_fields_in_exact_json() {
        let usage = Usage {
            input: 1,
            output: 2,
            cache_read: 3,
            cache_write: 4,
            cache_write_1h: 5,
            reasoning: 6,
            total_tokens: 7,
            cost: 0.125,
        };

        let value = serde_json::to_value(usage).unwrap();
        assert_eq!(
            value,
            serde_json::json!({
                "input": 1,
                "output": 2,
                "cache_read": 3,
                "cache_write": 4,
                "cache_write_1h": 5,
                "reasoning": 6,
                "total_tokens": 7,
                "cost": 0.125
            })
        );
        assert_eq!(serde_json::from_value::<Usage>(value).unwrap(), usage);
    }

    #[test]
    fn invalid_usage_field_types_are_rejected() {
        assert!(serde_json::from_str::<Usage>(r#"{"input":-1}"#).is_err());
        assert!(serde_json::from_str::<Usage>(r#"{"cost":"free"}"#).is_err());
    }
}
