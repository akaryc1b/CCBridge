use anyhow::Result;
#[cfg(not(feature = "desktop"))]
use anyhow::bail;
fn main() -> Result<()> {
    if ccbridge::cli::run_non_gui(std::env::args().skip(1).collect())? {
        return Ok(());
    }
    #[cfg(feature = "desktop")]
    {
        ccbridge::desktop::run()
    }
    #[cfg(not(feature = "desktop"))]
    {
        bail!("此构建仅提供 Runner，请使用 rpc、mcp 或 worker")
    }
}
