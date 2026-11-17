use std::path::PathBuf;

use crate::model::ModelRef;

/// 净化且不可变的进程内运行时配置快照。
///
/// 该类型不承诺 serde 或配置持久化格式，也不保存凭据和连接选项。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigSnapshot {
    /// 本次运行前已经选定的模型。
    pub model: ModelRef,
    /// 本次运行的工作目录。
    pub cwd: PathBuf,
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::ConfigSnapshot;
    use crate::model::ModelRef;

    #[test]
    fn config_snapshot_contains_only_selected_model_and_cwd() {
        let snapshot = ConfigSnapshot {
            model: ModelRef {
                provider: "anthropic".into(),
                model: "claude".into(),
            },
            cwd: PathBuf::from("/work/project"),
        };

        assert_eq!(snapshot.model.provider, "anthropic");
        assert_eq!(snapshot.model.model, "claude");
        assert_eq!(snapshot.cwd, PathBuf::from("/work/project"));
    }
}
