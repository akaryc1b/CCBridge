//! Fixed PowerShell bootstrap only; business operations go through Runner JSON RPC.
use crate::remote_platform::{powershell, ps_literal};
use anyhow::Result;
pub fn install(destination: &str, version: &str) -> Result<String> {
    Ok(powershell(&format!(
        r#"
$ErrorActionPreference='Stop'
$dest={dest}
$parent=Split-Path -Parent $dest
$sid=[System.Security.Principal.WindowsIdentity]::GetCurrent().User
function PrivateDirectory([string]$path) {{
  $created=$false
  if (!(Test-Path -LiteralPath $path)) {{
    $up=Split-Path -Parent $path
    if (!(Test-Path -LiteralPath $up)) {{ PrivateDirectory $up }}
    [System.IO.Directory]::CreateDirectory($path) | Out-Null
    $created=$true
  }}
  $entry=Get-Item -LiteralPath $path -Force
  $p=$entry
  while ($p) {{
    if (($p.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {{ throw '安装路径包含目录联接或符号链接，已停止操作' }}
    $p=$p.Parent
  }}
  $acl=[System.IO.Directory]::GetAccessControl($path)
  if ($created) {{ $acl.SetOwner($sid) }} elseif ($acl.GetOwner([System.Security.Principal.SecurityIdentifier]).Value -ne $sid.Value) {{ throw '安装目录不属于当前 Windows 账号，已停止操作' }}
  $acl.SetAccessRuleProtection($true,$false)
  foreach ($rule in @($acl.Access)) {{ $acl.RemoveAccessRuleSpecific($rule) }}
  foreach ($who in @($sid,[System.Security.Principal.SecurityIdentifier]::new('S-1-5-18'))) {{
    $rule=[System.Security.AccessControl.FileSystemAccessRule]::new($who,'FullControl','ContainerInherit,ObjectInherit','None','Allow')
    $acl.AddAccessRule($rule)
  }}
  [System.IO.Directory]::SetAccessControl($path,$acl)
}}
PrivateDirectory $parent
if (Test-Path -LiteralPath $dest) {{ throw '目标路径已有 Runner，未覆盖；请检测版本或使用新的版本目录' }}
$tmp=Join-Path $parent ([Guid]::NewGuid().ToString()+'.upload.exe')
try {{
  $file=[System.IO.File]::Open($tmp,[System.IO.FileMode]::CreateNew)
  try {{ [Console]::OpenStandardInput().CopyTo($file); $file.Flush($true) }} finally {{ $file.Dispose() }}
  $actual=& $tmp --version
  if ($LASTEXITCODE -ne 0 -or $actual -ne {expected}) {{ throw 'Runner 版本校验失败，原版本未替换' }}
  [System.IO.File]::Move($tmp,$dest)
}} finally {{ if (Test-Path -LiteralPath $tmp) {{ Remove-Item -LiteralPath $tmp }} }}
"#,
        dest = ps_literal(destination)?,
        expected = ps_literal(&format!("CCBridge {version}"))?
    )))
}

pub fn import_bundle(parent: &str, name: &str, url: &str) -> Result<String> {
    crate::remote_platform::windows_name(name)?;
    Ok(powershell(&format!(
        r#"
$ErrorActionPreference='Stop'
$root=[System.IO.Path]::GetFullPath({parent})
$home=[System.IO.Path]::GetFullPath($env:USERPROFILE).TrimEnd('\')+'\'
if (!$root.StartsWith($home,[System.StringComparison]::OrdinalIgnoreCase) -and $root.TrimEnd('\') -ne $home.TrimEnd('\')) {{ throw 'CCBRIDGE_OUTSIDE_HOME' }}
$dir=Get-Item -LiteralPath $root -Force
$p=$dir
while ($p) {{ if (($p.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {{ throw '安装路径包含目录联接或符号链接，已停止操作' }}; $p=$p.Parent }}
$dest=Join-Path $root {name}
if (Test-Path -LiteralPath $dest) {{ throw 'CCBRIDGE_ALREADY_EXISTS' }}
$tmp=Join-Path $root ('.ccbridge-bundle-'+[Guid]::NewGuid().ToString())
[System.IO.Directory]::CreateDirectory($tmp) | Out-Null
$bundle=Join-Path $tmp 'input.bundle'
$stage=Join-Path $tmp 'repository'
try {{
  $f=[System.IO.File]::Open($bundle,[System.IO.FileMode]::CreateNew)
  try {{ [Console]::OpenStandardInput().CopyTo($f) }} finally {{ $f.Dispose() }}
  & git -c core.hooksPath=NUL clone -- $bundle $stage 1>$null
  if ($LASTEXITCODE -ne 0) {{ throw 'Git bundle import failed' }}
  & git -C $stage remote set-url origin {url}
  if ($LASTEXITCODE -ne 0) {{ throw 'Git remote setup failed' }}
  [System.IO.Directory]::Move($stage,$dest)
}} finally {{ if (Test-Path -LiteralPath $tmp) {{ Remove-Item -LiteralPath $tmp -Recurse -Force }} }}
"#,
        parent = ps_literal(parent)?,
        name = ps_literal(name)?,
        url = ps_literal(url)?
    )))
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    #[test]
    fn native_powershell_bootstrap_and_repeat_install() -> Result<()> {
        let binary = std::env::var("CCBRIDGE_TEST_BIN")?;
        let root = std::env::temp_dir().join(format!("CCBridge 引导 {}", uuid::Uuid::new_v4()));
        let dest = root.join("runner").join("0.10.8").join("ccbridge.exe");
        let encoded = install(dest.to_str().unwrap(), env!("CARGO_PKG_VERSION"))?;
        let execute = |bytes: &[u8]| -> Result<std::process::Output> {
            let mut child = Command::new("powershell.exe")
                .args(encoded.split_whitespace().skip(1))
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?;
            let sent = child.stdin.take().unwrap().write_all(bytes);
            let output = child.wait_with_output()?;
            if output.status.success() {
                sent?;
            }
            Ok(output)
        };
        let bytes = std::fs::read(binary)?;
        let first = execute(&bytes)?;
        assert!(
            first.status.success(),
            "{}",
            String::from_utf8_lossy(&first.stderr)
        );
        let modified = dest.metadata()?.modified()?;
        let second = execute(&[])?;
        assert!(!second.status.success());
        assert_eq!(dest.metadata()?.modified()?, modified);
        assert_eq!(
            Command::new(&dest).arg("--version").output()?.stdout,
            format!("CCBridge {}\n", env!("CARGO_PKG_VERSION")).as_bytes()
        );
        let encoded = crate::remote_platform::runner_command(
            dest.to_str().unwrap(),
            root.join("状态").to_str().unwrap(),
        )?;
        let mut child = Command::new("powershell.exe")
            .args(encoded.split_whitespace().skip(1))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(br#"{"op":"probe","claude":""}"#)?;
        let output = child.wait_with_output()?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(value["ok"], true, "{value}");
        assert!(
            value["data"]["state_directory"]
                .as_str()
                .unwrap()
                .contains("状态")
        );
        std::fs::remove_dir_all(root)?;
        Ok(())
    }
}
