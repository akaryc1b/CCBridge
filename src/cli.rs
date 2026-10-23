#[cfg(unix)]
use crate::integration;
use crate::{common, databases, mcp, runner};
use anyhow::{Result, bail};
pub fn run_non_gui(args: Vec<String>) -> Result<bool> {
    #[cfg(unix)]
    unsafe {
        libc::umask(0o077);
    }
    match args.first().map(String::as_str) {
        Some("database-server") => databases::serve(
            args.get(1).ok_or_else(|| anyhow::anyhow!("缺少任务编号"))?,
            args.get(2).ok_or_else(|| anyhow::anyhow!("缺少连接编号"))?,
        ),
        Some("rpc") => runner::rpc_stdio(),
        Some("worker") => {
            #[cfg(windows)]
            if args.get(2).map(String::as_str) == Some("--owner-sid") {
                crate::platform::ensure_worker_owner(
                    args.get(3)
                        .ok_or_else(|| anyhow::anyhow!("缺少 Worker 账号校验"))?,
                )?;
            }
            runner::worker(args.get(1).ok_or_else(|| anyhow::anyhow!("缺少任务编号"))?)
        }
        Some("mcp") => {
            #[cfg(windows)]
            bail!("此构建是 Windows 远端 Runner；请在 Mac App 配置 Codex MCP");
            #[cfg(unix)]
            tokio::runtime::Runtime::new()?.block_on(mcp::serve_bridge())
        }
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
            #[cfg(windows)]
            bail!("远端 Runner 无需写入 Codex 配置，请在 Mac App 配置 MCP");
            #[cfg(unix)]
            {
                println!("{}", integration::configure()?);
                Ok(())
            }
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
