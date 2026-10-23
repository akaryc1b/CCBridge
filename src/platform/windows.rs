use anyhow::{Context, Result, bail};
use std::{
    os::windows::{
        ffi::OsStrExt,
        fs::MetadataExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
    },
    path::Path,
};
use windows_sys::Win32::{
    Foundation::*,
    Security::{Authorization::*, *},
    Storage::FileSystem::*,
    System::Threading::*,
};
fn wide(s: &std::ffi::OsStr) -> Vec<u16> {
    s.encode_wide().chain(Some(0)).collect()
}
struct Local(*mut core::ffi::c_void);
impl Drop for Local {
    fn drop(&mut self) {
        unsafe {
            LocalFree(self.0);
        }
    }
}
fn token() -> Result<OwnedHandle> {
    let mut h = std::ptr::null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut h) } == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(unsafe { OwnedHandle::from_raw_handle(h) })
}
fn with_sid<T>(f: impl FnOnce(PSID) -> Result<T>) -> Result<T> {
    with_token_sid(TokenUser, f)
}
fn with_token_sid<T>(
    class: TOKEN_INFORMATION_CLASS,
    f: impl FnOnce(PSID) -> Result<T>,
) -> Result<T> {
    let token = token()?;
    let mut len = 0;
    unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            class,
            std::ptr::null_mut(),
            0,
            &mut len,
        );
    }
    // TOKEN_USER needs pointer alignment; Vec<usize> provides it.
    let mut data = vec![0usize; (len as usize).div_ceil(std::mem::size_of::<usize>())];
    if len == 0
        || unsafe {
            GetTokenInformation(
                token.as_raw_handle(),
                class,
                data.as_mut_ptr().cast(),
                len,
                &mut len,
            )
        } == 0
    {
        return Err(std::io::Error::last_os_error().into());
    }
    let sid = unsafe {
        if class == TokenUser {
            (*(data.as_ptr().cast::<TOKEN_USER>())).User.Sid
        } else {
            (*(data.as_ptr().cast::<TOKEN_OWNER>())).Owner
        }
    };
    f(sid)
}
fn reject_reparse(path: &Path) -> Result<()> {
    for ancestor in path.ancestors() {
        let m = std::fs::symlink_metadata(ancestor)?;
        if m.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            bail!("私有存储路径不能包含符号链接或目录联接");
        }
    }
    Ok(())
}
fn check_owner(path: &Path, allow_default: bool) -> Result<()> {
    reject_reparse(path)?;
    with_sid(|sid| {
        let path = wide(path.as_os_str());
        let mut owner = std::ptr::null_mut();
        let mut descriptor = std::ptr::null_mut();
        let status = unsafe {
            GetNamedSecurityInfoW(
                path.as_ptr(),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION,
                &mut owner,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut descriptor,
            )
        };
        if status != 0 {
            return Err(std::io::Error::from_raw_os_error(status as i32).into());
        }
        let _descriptor = Local(descriptor);
        if unsafe { EqualSid(owner, sid) } == 0
            && !(allow_default
                && with_token_sid(TokenOwner, |default| {
                    Ok(unsafe { EqualSid(owner, default) } != 0)
                })?)
        {
            bail!("私有存储不属于当前 Windows 账号，已停止操作");
        }
        Ok(())
    })
}
/// Global Git configuration contains no token; respect the token's standard owner, including elevated administrators.
pub fn account_file(path: &Path) -> Result<()> {
    check_owner(path, true)
}
pub fn private_file(path: &Path) -> Result<()> {
    check_owner(path, false)?;
    with_sid(|sid| {
        let path = wide(path.as_os_str());
        let mut text = std::ptr::null_mut();
        if unsafe { ConvertSidToStringSidW(sid, &mut text) } == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let _text = Local(text.cast());
        let mut len = 0;
        unsafe {
            while *text.add(len) != 0 {
                len += 1;
            }
        }
        let sid = String::from_utf16(unsafe { std::slice::from_raw_parts(text, len) })?;
        let sddl = wide(std::ffi::OsStr::new(&format!(
            "D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;{sid})"
        )));
        let mut sd = std::ptr::null_mut();
        if unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut sd,
                std::ptr::null_mut(),
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error().into());
        }
        let _sd = Local(sd);
        if unsafe {
            SetFileSecurityW(
                path.as_ptr(),
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                sd,
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(())
    })
}
/// Only call for a path exclusively created by this operation, never an existing foreign file.
pub fn created_private(path: &Path) -> Result<()> {
    reject_reparse(path)?;
    with_sid(|sid| {
        let mut name = wide(path.as_os_str());
        let status = unsafe {
            SetNamedSecurityInfoW(
                name.as_mut_ptr(),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION,
                sid,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
            )
        };
        if status != 0 {
            return Err(std::io::Error::from_raw_os_error(status as i32).into());
        }
        Ok(())
    })?;
    private_file(path)
}
pub fn private_dir(path: &Path) -> Result<()> {
    if !path.exists() {
        let parent = path.parent().context("私有目录缺少父目录")?;
        if !parent.exists() {
            private_dir(parent)?;
        }
        reject_reparse(parent)?;
        std::fs::create_dir(path)?;
        return created_private(path);
    }
    if !path.is_dir() {
        bail!("私有目录路径被文件占用");
    }
    private_file(path)
}
pub fn replace(from: &Path, to: &Path) -> Result<()> {
    move_with_flags(from, to, MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)
}
pub fn move_exclusive(from: &Path, to: &Path) -> Result<()> {
    move_with_flags(from, to, MOVEFILE_WRITE_THROUGH)
}
fn move_with_flags(from: &Path, to: &Path, flags: u32) -> Result<()> {
    let (from, to) = (wide(from.as_os_str()), wide(to.as_os_str()));
    if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), flags) } == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}
pub fn alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    let h = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if h.is_null() {
        return false;
    }
    let h = unsafe { OwnedHandle::from_raw_handle(h) };
    let mut code = 0;
    unsafe { GetExitCodeProcess(h.as_raw_handle(), &mut code) != 0 && code == STILL_ACTIVE as u32 }
}

fn current_sid() -> Result<String> {
    with_sid(|sid| {
        let mut text = std::ptr::null_mut();
        if unsafe { ConvertSidToStringSidW(sid, &mut text) } == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let _text = Local(text.cast());
        let mut len = 0;
        unsafe {
            while *text.add(len) != 0 {
                len += 1;
            }
        }
        Ok(String::from_utf16(unsafe {
            std::slice::from_raw_parts(text, len)
        })?)
    })
}
pub fn ensure_worker_owner(expected: &str) -> Result<()> {
    if current_sid()? != expected {
        bail!("Worker 启动账号不一致，未执行任务");
    }
    Ok(())
}
pub fn worker_log(dir: &Path) -> Result<std::fs::File> {
    use windows_sys::Win32::System::Console::{STD_ERROR_HANDLE, STD_OUTPUT_HANDLE, SetStdHandle};
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("worker.log"))?;
    for handle in [STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
        if unsafe { SetStdHandle(handle, log.as_raw_handle()) } == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
    }
    Ok(log)
}
/// WMI brokers creation outside the SSH/CI parent job. No service install, SYSTEM task or password.
pub fn spawn_worker(id: &str, dir: &Path) -> Result<u32> {
    use crate::common::{coded, valid_id};
    use crate::remote_platform::{powershell, ps_literal};
    valid_id(id)?;
    let exe = std::env::current_exe()?;
    let exe = exe.to_str().context("Runner 路径编码无效")?;
    // Win32 filenames cannot contain quotes. Only the executable path and validated UUID/SID occur here.
    if exe.contains(['"', '\r', '\n', '\0']) {
        bail!("Runner 路径无效");
    }
    let sid = current_sid()?;
    let command = format!("\"{exe}\" worker {id} --owner-sid {sid}");
    let script = format!(
        r#"
$ErrorActionPreference='Stop'
$startup=([wmiclass]'Win32_ProcessStartup').CreateInstance()
$startup.ShowWindow=0
$startup.CreateFlags=16778760
$startup.EnvironmentVariables=@([System.Environment]::GetEnvironmentVariables().GetEnumerator() | ForEach-Object {{ [string]$_.Key+'='+[string]$_.Value }})
$process=[wmiclass]'Win32_Process'
$process.Scope.Options.Impersonation=[System.Management.ImpersonationLevel]::Impersonate
$result=$process.Create({command},{directory},$startup)
if ($result.ReturnValue -ne 0) {{ throw ('Windows WMI 创建 Worker 失败，代码：'+$result.ReturnValue) }}
[Console]::WriteLine([string]$result.ProcessId)
"#,
        command = ps_literal(&command)?,
        directory = ps_literal(dir.to_str().context("任务目录编码无效")?)?
    );
    let encoded = powershell(&script);
    let output = std::process::Command::new("powershell.exe")
        .args(encoded.split_whitespace().skip(1))
        .output()
        .context("无法启动 Windows Worker 引导")?;
    if !output.status.success() {
        return Err(coded(
            "worker_launch_failed",
            format!(
                "Windows 独立任务启动失败：{}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        ));
    }
    let pid = String::from_utf8(output.stdout)?
        .trim()
        .parse::<u32>()
        .context("Windows Worker 未返回有效进程编号")?;
    if pid == 0 {
        bail!("Windows Worker 未启动");
    }
    Ok(pid)
}
