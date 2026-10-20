//! Bounded UI credential operations. OS access control is honored; no duplicate authorization prompts.
use anyhow::{Context, Result, bail};
use std::sync::{Arc, OnceLock};
fn gate() -> Arc<tokio::sync::Semaphore> {
    static GATE: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();
    GATE.get_or_init(|| Arc::new(tokio::sync::Semaphore::new(1)))
        .clone()
}
pub fn pending() -> bool {
    gate().available_permits() == 0
}
pub async fn call<T: Send + 'static>(f: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    call_with(gate(), std::time::Duration::from_secs(30), f).await
}
async fn call_with<T: Send + 'static>(
    gate: Arc<tokio::sync::Semaphore>,
    timeout: std::time::Duration,
    f: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    let permit = gate.try_acquire_owned().map_err(|_| {
        anyhow::anyhow!("已有钥匙串授权正在等待，请先完成或取消系统提示；不要重复提交")
    })?;
    let task = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        f()
    });
    match tokio::time::timeout(timeout, task).await {
        Ok(r) => r.context("钥匙串处理线程异常结束")?,
        Err(_) => bail!(
            "钥匙串授权尚未完成，请处理 macOS 系统提示后再操作。若只需登录，也可手动输入 SSH 密码并关闭记住密码"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn bounded_without_duplicate_prompts() {
        let gate = Arc::new(tokio::sync::Semaphore::new(1));
        let err = call_with(gate.clone(), std::time::Duration::from_millis(5), || {
            std::thread::sleep(std::time::Duration::from_millis(80));
            Ok(1)
        })
        .await
        .unwrap_err();
        assert!(err.to_string().contains("钥匙串授权尚未完成"));
        assert_eq!(gate.available_permits(), 0);
        assert!(
            call_with(gate.clone(), std::time::Duration::from_secs(1), || Ok(2))
                .await
                .unwrap_err()
                .to_string()
                .contains("已有钥匙串授权")
        );
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert_eq!(
            call_with(gate, std::time::Duration::from_secs(1), || Ok(3))
                .await
                .unwrap(),
            3
        );
    }
}
