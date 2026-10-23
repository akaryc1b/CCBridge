//! Remote path syntax is evaluated independently of the Mac client's host OS.
use anyhow::{Result, bail};
use base64::Engine;

pub fn windows_absolute(path: &str) -> bool {
    let path = path.strip_prefix(r"\\?\").unwrap_or(path);
    let b = path.as_bytes();
    b.len() >= 3
        && b[0].is_ascii_alphabetic()
        && b[1] == b':'
        && matches!(b[2], b'/' | b'\\')
        && !path.contains(['\0', '\n', '\r'])
        && !path[3..]
            .split(['/', '\\'])
            .any(|c| c == ".." || c.contains(':'))
}
pub fn absolute(path: &str) -> bool {
    !path.contains(['\0', '\n', '\r']) && (path.starts_with('/') || windows_absolute(path))
}
pub fn powershell(script: &str) -> String {
    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    format!(
        "powershell.exe -NoLogo -NoProfile -NonInteractive -EncodedCommand {}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}
pub fn ps_literal(value: &str) -> Result<String> {
    if value.contains(['\0', '\n', '\r']) {
        bail!("路径不能包含换行或空字符");
    }
    Ok(format!("'{}'", value.replace('\'', "''")))
}
pub fn windows_name(name: &str) -> Result<()> {
    let base = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    if name.is_empty()
        || name.ends_with(['.', ' '])
        || name
            .chars()
            .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
        || ["CON", "PRN", "AUX", "NUL", "CLOCK$", "CONIN$", "CONOUT$"].contains(&base.as_str())
        || (base.len() == 4
            && (base.starts_with("COM") || base.starts_with("LPT"))
            && matches!(base.as_bytes()[3], b'1'..=b'9'))
    {
        bail!("名称包含 Windows 不支持的字符或保留名称");
    }
    Ok(())
}
pub const DISCOVER_WINDOWS: &str = r#"
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)
$c = Get-Command claude.exe -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
$claude = if ($c) { $c.Source } else { '' }
if (!$claude) {
  $candidate = Join-Path $env:USERPROFILE '.local\bin\claude.exe'
  if (Test-Path -LiteralPath $candidate -PathType Leaf) { $claude = $candidate }
}
[ordered]@{ home=$env:USERPROFILE; user=[System.Security.Principal.WindowsIdentity]::GetCurrent().Name; claude=$claude; platform=('Windows ' + [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()); local_app_data=$env:LOCALAPPDATA } | ConvertTo-Json -Compress
"#;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn remote_paths_are_not_host_paths() {
        for p in [
            "C:\\Users\\测试 用户",
            "D:/Projects/app",
            "/home/cyb/projects",
        ] {
            assert!(absolute(p), "{p}");
        }
        for p in [
            "C:relative",
            "\\\\server\\share",
            "C:\\x\\..\\secret",
            "C:\\x:stream",
            "C:\\x\n",
            "relative",
        ] {
            assert!(!absolute(p), "{p}");
        }
    }
    #[test]
    fn encoded_script_has_no_shell_arguments() {
        let script = "Write-Output '测试 & path'";
        let cmd = powershell(script);
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(cmd.split_whitespace().last().unwrap())
            .unwrap();
        let units: Vec<_> = decoded
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        assert_eq!(String::from_utf16(&units).unwrap(), script);
        assert_eq!(
            ps_literal("C:\\it's & files").unwrap(),
            "'C:\\it''s & files'"
        );
    }
    #[test]
    fn windows_names() {
        for p in ["con.txt", "LPT1", "a:b", "a.", "a ", "a\\b", "NUL"] {
            assert!(windows_name(p).is_err(), "{p}");
        }
        for p in ["项目.txt", "normal name", "COM10"] {
            assert!(windows_name(p).is_ok(), "{p}");
        }
    }
}

/// Lexical client-side scope check; Runner always rechecks the canonical OS path.
pub fn within(path: &str, root: &str) -> bool {
    if windows_absolute(root) {
        let norm = |s: &str| {
            s.strip_prefix(r"\\?\")
                .unwrap_or(s)
                .replace('\\', "/")
                .trim_end_matches('/')
                .to_lowercase()
        };
        if !windows_absolute(path) {
            return false;
        }
        let (p, r) = (norm(path), norm(root));
        p == r || p.starts_with(&(r + "/"))
    } else {
        std::path::Path::new(path).starts_with(root)
    }
}
