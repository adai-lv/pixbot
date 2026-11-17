use crate::plan::Plan;
use clap::Parser;

#[derive(Parser)]
#[command(
    name = "pixbot",
    version,
    about = "AI Agent CLI",
    long_about = "Pixbot AI Agent CLI。支持单发执行；不带-p参数时进入全屏TUI（-r/--repl 切回行式REPL）。"
)]
#[group(multiple = false)]
struct Args {
    /// 行式模式：REPL（默认为全屏 TUI）
    #[arg(short = 'r', long = "repl")]
    pub repl: bool,

    /// 单发模式：执行 prompt 后退出（无交互）
    #[arg(short = 'p', long = "prompt")]
    pub prompt: Option<String>,

    /// 升级 pixbot 到指定版本，默认最新版本
    #[arg(short = 'u', long = "upgrade")]
    pub upgrade: Option<String>,
}

/// 纯映射：命令行参数 → 执行计划。无 IO、无 async，全部分支判断集中于此。
fn plan(args: Args) -> Plan {
    if let Some(upgrade) = args.upgrade {
        return Plan::Upgrade(upgrade);
    }

    if let Some(prompt) = args.prompt {
        return Plan::Headless(prompt);
    }

    if args.repl { Plan::Repl } else { Plan::Tui }
}

pub async fn exec() -> anyhow::Result<()> {
    let start = std::time::Instant::now();
    let plan = plan(Args::parse());
    plan.execute(start).await
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn repl_default_is_tui() {
        assert!(Args::try_parse_from(["pixbot"]).is_ok());
    }

    #[test]
    fn repl_short_flag_is_required() {
        assert!(Args::try_parse_from(["pixbot", "-r"]).is_ok());
    }

    #[test]
    fn repl_long_flag_is_required() {
        assert!(Args::try_parse_from(["pixbot", "--repl"]).is_ok());
    }

    #[test]
    fn prompt_short_flag_is_required() {
        assert!(Args::try_parse_from(["pixbot", "-p", "hello"]).is_ok());
    }

    #[test]
    fn prompt_long_flag_is_required() {
        assert!(Args::try_parse_from(["pixbot", "--prompt", "hello"]).is_ok());
    }

    #[test]
    fn prompt_default_is_none() {
        assert!(Args::try_parse_from(["pixbot"]).unwrap().prompt.is_none());
    }

    #[test]
    fn tui_long_flag_is_rejected() {
        assert!(Args::try_parse_from(["pixbot", "--tui"]).is_err());
    }

    #[test]
    fn tui_short_flag_is_rejected() {
        assert!(Args::try_parse_from(["pixbot", "-t"]).is_err());
    }
}
