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
    let token = token()?;
    let mut len = 0;
    unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
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
                TokenUser,
                data.as_mut_ptr().cast(),
                len,
                &mut len,
            )
        } == 0
    {
        return Err(std::io::Error::last_os_error().into());
    }
    f(unsafe { (*(data.as_ptr().cast::<TOKEN_USER>())).User.Sid })
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
pub fn private_file(path: &Path) -> Result<()> {
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
        if unsafe { EqualSid(owner, sid) } == 0 {
            bail!("私有存储不属于当前 Windows 账号，已停止操作");
        }
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
pub fn private_dir(path: &Path) -> Result<()> {
    if !path.exists() {
        let parent = path.parent().context("私有目录缺少父目录")?;
        if !parent.exists() {
            private_dir(parent)?;
        }
        reject_reparse(parent)?;
        std::fs::create_dir(path)?;
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
