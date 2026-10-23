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
  if (!(Test-Path -LiteralPath $path)) {{
    $up=Split-Path -Parent $path
    if (!(Test-Path -LiteralPath $up)) {{ PrivateDirectory $up }}
    [System.IO.Directory]::CreateDirectory($path) | Out-Null
  }}
  $entry=Get-Item -LiteralPath $path -Force
  $p=$entry
  while ($p) {{
    if (($p.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {{ throw 'CCBRIDGE_PATH_CONFLICT' }}
    $p=$p.Parent
  }}
  $acl=Get-Acl -LiteralPath $path
  if ($acl.GetOwner([System.Security.Principal.SecurityIdentifier]).Value -ne $sid.Value) {{ throw 'CCBRIDGE_OWNER_CONFLICT' }}
  $acl.SetAccessRuleProtection($true,$false)
  foreach ($rule in @($acl.Access)) {{ $acl.RemoveAccessRuleSpecific($rule) }}
  foreach ($who in @($sid,[System.Security.Principal.SecurityIdentifier]::new('S-1-5-18'))) {{
    $rule=[System.Security.AccessControl.FileSystemAccessRule]::new($who,'FullControl','ContainerInherit,ObjectInherit','None','Allow')
    $acl.AddAccessRule($rule)
  }}
  Set-Acl -LiteralPath $path -AclObject $acl
}}
PrivateDirectory $parent
if (Test-Path -LiteralPath $dest) {{ throw 'Runner target exists; refusing to replace an executable' }}
$tmp=Join-Path $parent ([Guid]::NewGuid().ToString()+'.upload.exe')
try {{
  $file=[System.IO.File]::Open($tmp,[System.IO.FileMode]::CreateNew)
  try {{ [Console]::OpenStandardInput().CopyTo($file); $file.Flush($true) }} finally {{ $file.Dispose() }}
  $actual=& $tmp --version
  if ($LASTEXITCODE -ne 0 -or $actual -ne {expected}) {{ throw 'Runner version verification failed' }}
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
while ($p) {{ if (($p.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {{ throw 'CCBRIDGE_PATH_CONFLICT' }}; $p=$p.Parent }}
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
