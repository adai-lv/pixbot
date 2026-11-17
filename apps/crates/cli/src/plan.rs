pub enum Plan {
    /// 全屏 TUI（默认）
    Tui,
    /// 交互 REPL（-r/--repl 显式选择）
    Repl,
    /// headless 单发（携带 prompt）
    Headless(String),
    /// upgrade 子命令：升级到最新版本（携带版本号）
    Upgrade(String),
}

impl Plan {
    pub async fn execute(self, start: std::time::Instant) -> anyhow::Result<()> {
        match self {
            Plan::Tui => {
                println!("TUI");
            }
            Plan::Repl => {
                println!("REPL");
            }
            Plan::Headless(prompt) => {
                println!("HEADLESS: {}", prompt);
            }
            Plan::Upgrade(version) => {
                println!("UPGRADE: {}", version);
            }
        }
        let duration = start.elapsed();
        println!("Duration: {:?}", duration);
        Ok(())
    }
}
