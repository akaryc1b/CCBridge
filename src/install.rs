//! Install transaction. Backups stay on the target volume until explicit recovery/cleanup.
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
pub fn bundle() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()?
        .ancestors()
        .find(|p| p.extension().is_some_and(|e| e == "app"))
        .map(Path::to_path_buf)
}
pub fn location() -> Value {
    let source = bundle();
    json!({"installed":source.as_deref()==Some(Path::new("/Applications/CCBridge.app")),"source":source,"target":"/Applications/CCBridge.app","target_exists":Path::new("/Applications/CCBridge.app").exists()})
}
pub fn replace_with(
    source: &Path,
    target: &Path,
    copy: impl FnOnce(&Path, &Path) -> Result<()>,
) -> Result<PathBuf> {
    if target
        .symlink_metadata()
        .is_ok_and(|m| m.file_type().is_symlink())
    {
        bail!("安装目标是符号链接，请在Finder核对")
    }
    let parent = target.parent().context("安装目标无效")?;
    let stage = parent.join(format!(".CCBridge-install-{}.app", uuid::Uuid::new_v4()));
    let backup = parent.join(format!(".CCBridge-backup-{}.app", uuid::Uuid::new_v4()));
    if let Err(e) = copy(source, &stage) {
        let _ = std::fs::remove_dir_all(&stage);
        return Err(e.context("复制或验证失败，原App未替换"));
    }
    let had = target.exists();
    if had {
        if let Err(e) = std::fs::rename(target, &backup) {
            let _ = std::fs::remove_dir_all(&stage);
            return Err(anyhow::Error::from(e).context("无法保留旧版App，未替换"));
        }
    }
    if let Err(e) = std::fs::rename(&stage, target) {
        let rollback = if had {
            std::fs::rename(&backup, target)
        } else {
            Ok(())
        };
        let _ = std::fs::remove_dir_all(&stage);
        if let Err(r) = rollback {
            bail!(
                "安装失败：{e}；旧版位于{}，请在Finder恢复：{r}",
                backup.display()
            )
        }
        return Err(anyhow::Error::from(e).context("安装失败，已恢复原App"));
    }
    Ok(backup)
}
#[cfg(target_os = "macos")]
pub fn install() -> Result<()> {
    let source = bundle().context("当前不是App运行环境")?;
    let target = Path::new("/Applications/CCBridge.app");
    if source == target {
        return Ok(());
    }
    if target.exists() {
        let output = std::process::Command::new("/usr/libexec/PlistBuddy")
            .args(["-c", "Print :CFBundleShortVersionString"])
            .arg(target.join("Contents/Info.plist"))
            .output()?;
        let installed = String::from_utf8(output.stdout)?;
        let version = |s: &str| -> Option<Vec<u32>> {
            s.trim()
                .split('.')
                .map(str::parse)
                .collect::<std::result::Result<Vec<u32>, _>>()
                .ok()
        };
        if !output.status.success()
            || version(installed.trim()).is_none()
            || version(installed.trim()) < version(env!("CARGO_PKG_VERSION"))
        {
            bail!(
                "应用程序中已有旧版CCBridge。请退出后在Finder拖入替换，或打开已安装副本使用应用内更新；未覆盖旧版"
            )
        }
        if !std::process::Command::new("/usr/bin/codesign")
            .args(["--verify", "--deep", "--strict"])
            .arg(target)
            .status()?
            .success()
        {
            bail!("已安装副本签名验证失败，请在Finder核对")
        }
        return Ok(());
    }
    replace_with(&source, target, |src, dst| {
        if !std::process::Command::new("/usr/bin/ditto")
            .arg(src)
            .arg(dst)
            .status()?
            .success()
        {
            bail!("没有写入应用程序的权限，请在Finder拖入安装")
        }
        if !std::process::Command::new("/usr/bin/codesign")
            .args(["--verify", "--deep", "--strict"])
            .arg(dst)
            .status()?
            .success()
        {
            bail!("App签名验证失败")
        }
        Ok(())
    })?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_stage_preserves_target() {
        let p = std::env::temp_dir().join(format!("ccb-install-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&p).unwrap();
        let target = p.join("old.app");
        std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join("keep"), "old").unwrap();
        assert!(
            replace_with(&target, &target, |_, stage| {
                std::fs::create_dir(stage)?;
                bail!("injected copy failure")
            })
            .is_err()
        );
        assert_eq!(std::fs::read_to_string(target.join("keep")).unwrap(), "old");
        assert_eq!(std::fs::read_dir(&p).unwrap().count(), 1);
        std::fs::remove_dir_all(p).unwrap();
    }
}
