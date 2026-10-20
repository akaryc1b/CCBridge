use crate::{common, databases, integration, mcp, runner};
use anyhow::{Result, bail};
pub fn run_non_gui(args: Vec<String>) -> Result<bool> {
    unsafe {
        libc::umask(0o077);
    }
    match args.first().map(String::as_str) {
        Some("database-server") => databases::serve(
            args.get(1).ok_or_else(|| anyhow::anyhow!("缺少任务编号"))?,
            args.get(2).ok_or_else(|| anyhow::anyhow!("缺少连接编号"))?,
        ),
        Some("rpc") => runner::rpc_stdio(),
        Some("worker") => runner::worker(
            args.get(1)
                .ok_or_else(|| anyhow::anyhow!("missing task id"))?,
        ),
        Some("mcp") => tokio::runtime::Runtime::new()?.block_on(mcp::serve_bridge()),
        Some("permission-server") => {
            tokio::runtime::Runtime::new()?.block_on(mcp::serve_permissions(
                args.get(1)
                    .ok_or_else(|| anyhow::anyhow!("missing task id"))?,
            ))
        }
        Some("request") => tokio::runtime::Runtime::new()?.block_on(async {
            use std::io::Read;
            let mut s = String::new();
            std::io::stdin().read_to_string(&mut s)?;
            let v = common::local_call(serde_json::from_str(&s)?).await?;
            println!("{}", serde_json::to_string_pretty(&v)?);
            Ok(())
        }),
        Some("configure-codex") => {
            println!("{}", integration::configure()?);
            Ok(())
        }
        Some("--version") => {
            println!("CCBridge {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        None | Some("app") => return Ok(false),
        _ => bail!("Usage: ccbridge [app | mcp | rpc | request | --version]"),
    }?;
    Ok(true)
}
