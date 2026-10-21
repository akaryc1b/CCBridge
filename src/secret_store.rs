//! Opt-in account-local storage. This is not protection from other processes of the same user.
use crate::common::*;
use anyhow::{Result, bail};
use std::{
    collections::BTreeMap,
    fs::OpenOptions,
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::Path,
};
fn check(path: &Path, directory: bool) -> Result<()> {
    let m = std::fs::symlink_metadata(path)?;
    if m.file_type().is_symlink()
        || m.uid() != unsafe { libc::geteuid() }
        || m.permissions().mode() & 0o077 != 0
        || (directory && !m.is_dir())
        || (!directory && !m.is_file())
    {
        bail!("本机凭据目录或文件权限不安全；请检查所属账号及0700/0600权限")
    }
    Ok(())
}
fn directory(root: &Path) -> Result<std::path::PathBuf> {
    let dir = root.join("secrets");
    if !dir.exists() {
        std::fs::DirBuilder::new().mode(0o700).create(&dir)?;
    }
    check(&dir, true)?;
    Ok(dir)
}
use std::os::unix::fs::DirBuilderExt;
fn read(root: &Path) -> Result<BTreeMap<String, String>> {
    let path = directory(root)?.join("passwords.json");
    match std::fs::symlink_metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(e) => return Err(e.into()),
        Ok(_) => {}
    }

    check(&path, false)?;
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    let m = file.metadata()?;
    if m.uid() != unsafe { libc::geteuid() } || m.permissions().mode() & 0o077 != 0 {
        bail!("本机凭据权限不安全")
    }
    let mut bytes = vec![];
    file.read_to_end(&mut bytes)?;
    Ok(serde_json::from_slice(&bytes)?)
}
pub fn load_at(root: &Path, key: &str) -> Result<Option<String>> {
    Ok(read(root)?.remove(key))
}
static WRITE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
pub fn update_at(root: &Path, key: &str, value: Option<&str>) -> Result<()> {
    let _lock = WRITE_LOCK
        .lock()
        .map_err(|_| anyhow::anyhow!("凭据保存锁异常，请重启App"))?;
    let mut all = read(root)?;
    match value {
        Some(v) => {
            all.insert(key.into(), v.into());
        }
        None => {
            all.remove(key);
        }
    }
    let dir = directory(root)?;
    let tmp = dir.join(format!("{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| -> Result<()> {
        let mut f = OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&tmp)?;
        f.write_all(&serde_json::to_vec(&all)?)?;
        f.sync_all()?;
        std::fs::rename(&tmp, dir.join("passwords.json"))?;
        std::fs::File::open(&dir)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}
pub fn load(key: &str) -> Result<Option<String>> {
    load_at(&app_dir()?, key)
}
pub fn update(key: &str, value: Option<&str>) -> Result<()> {
    update_at(&app_dir()?, key, value)
}
fn consented_at(root: &Path) -> bool {
    read_json::<serde_json::Value>(&root.join("local-storage-consent.json"))
        .is_ok_and(|v| v["version"] == 1)
}
pub fn consented() -> bool {
    app_dir().is_ok_and(|root| consented_at(&root))
}
fn confirm_at(root: &Path, confirmed: bool) -> Result<()> {
    if consented_at(root) {
        return Ok(());
    }
    if !confirmed {
        bail!("请先确认本机保存的安全提示")
    }
    write_json(
        &root.join("local-storage-consent.json"),
        &serde_json::json!({"version":1}),
    )
}
pub fn confirm_consent(confirmed: bool) -> Result<()> {
    confirm_at(&app_dir()?, confirmed)
}
pub fn validate_store(store: &str) -> Result<()> {
    if !["keychain", "file"].contains(&store) {
        bail!("保存方式无效")
    };
    Ok(())
}
#[cfg(all(feature = "bridge", target_os = "macos"))]
pub async fn get(store: &str, service: &str, key: &str) -> Result<String> {
    validate_store(store)?;
    if store == "file" {
        return load(&format!("{service}:{key}"))?
            .ok_or_else(|| anyhow::anyhow!("尚未在本机保存此账号密码，请在App重新输入保存"));
    }
    let (service, key) = (service.to_string(), key.to_string());
    crate::credentials::call(move || Ok(keyring::Entry::new(&service, &key)?.get_password()?)).await
}
#[cfg(all(feature = "bridge", target_os = "macos"))]
pub async fn set(store: &str, service: &str, key: &str, value: String) -> Result<()> {
    validate_store(store)?;
    if store == "file" {
        let dir = directory(&app_dir()?)?;
        let status = std::process::Command::new("/usr/bin/tmutil")
            .arg("addexclusion")
            .arg(&dir)
            .output()?;
        if !status.status.success() {
            bail!("无法将本机凭据目录排除出Time Machine备份；本次未保存，请检查系统备份权限")
        }
        update(&format!("{service}:{key}"), Some(&value))?;
        return Ok(());
    }
    bail!("仅支持本机保存，钥匙串只用于显式迁移旧凭据")
}
#[cfg(all(feature = "bridge", target_os = "macos"))]
pub async fn forget(service: &str, key: &str) -> Result<()> {
    update(&format!("{service}:{key}"), None)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn risk_confirmation_is_persistent_and_once() {
        let root = std::env::temp_dir().join(format!("ccb-consent-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        assert!(!consented_at(&root));
        assert!(confirm_at(&root, false).is_err());
        confirm_at(&root, true).unwrap();
        assert!(consented_at(&root));
        confirm_at(&root, false).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn private_atomic_scoped_store() {
        let root = std::env::temp_dir().join(format!("ccb-secret-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        update_at(&root, "ssh:a", Some("fixture-secret")).unwrap();
        update_at(&root, "git:b", Some("other")).unwrap();
        assert_eq!(
            load_at(&root, "ssh:a").unwrap().as_deref(),
            Some("fixture-secret")
        );
        assert_eq!(
            std::fs::metadata(root.join("secrets/passwords.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        update_at(&root, "ssh:a", None).unwrap();
        assert!(load_at(&root, "ssh:a").unwrap().is_none());
        assert_eq!(load_at(&root, "git:b").unwrap().as_deref(), Some("other"));
        std::fs::set_permissions(
            root.join("secrets/passwords.json"),
            std::fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        assert!(load_at(&root, "git:b").is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
