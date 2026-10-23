//! OS primitives used by the native Runner. No credentials or shell interpolation here.
use anyhow::Result;
use std::{
    path::Path,
    process::{Child, Command},
};

#[cfg(unix)]
pub fn private_dir(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    Ok(())
}
#[cfg(unix)]
pub fn private_file(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}
#[cfg(unix)]
pub fn replace(from: &Path, to: &Path) -> Result<()> {
    Ok(std::fs::rename(from, to)?)
}
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::{alive, move_exclusive, private_dir, private_file, replace};

#[cfg(unix)]
pub fn alive(pid: u32) -> bool {
    pid > 0 && unsafe { libc::kill(pid as i32, 0) == 0 }
}

/// Only the long-lived Worker detaches; its Claude children belong to its own process tree.
pub fn detach(command: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() < 0 {
                    Err(std::io::Error::last_os_error())
                } else {
                    Ok(())
                }
            });
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use windows_sys::Win32::System::Threading::{
            CREATE_BREAKAWAY_FROM_JOB, CREATE_NEW_PROCESS_GROUP, DETACHED_PROCESS,
        };
        command.creation_flags(
            CREATE_BREAKAWAY_FROM_JOB | DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP,
        );
    }
}

pub struct ProcessTree {
    #[cfg(unix)]
    child: Child,
    #[cfg(windows)]
    child: Box<dyn process_wrap::std::ChildWrapper>,
}
impl ProcessTree {
    pub fn spawn(mut command: Command) -> Result<Self> {
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
            Ok(Self {
                child: command.spawn()?,
            })
        }
        #[cfg(windows)]
        {
            use process_wrap::std::{CommandWrap, JobObject};
            Ok(Self {
                child: CommandWrap::from(command).wrap(JobObject).spawn()?,
            })
        }
    }
    pub fn id(&self) -> u32 {
        self.child.id()
    }
    pub fn stdin(&mut self) -> Option<std::process::ChildStdin> {
        #[cfg(unix)]
        {
            self.child.stdin.take()
        }
        #[cfg(windows)]
        {
            self.child.stdin().take()
        }
    }
    pub fn stdout(&mut self) -> Option<std::process::ChildStdout> {
        #[cfg(unix)]
        {
            self.child.stdout.take()
        }
        #[cfg(windows)]
        {
            self.child.stdout().take()
        }
    }
    pub fn stderr(&mut self) -> Option<std::process::ChildStderr> {
        #[cfg(unix)]
        {
            self.child.stderr.take()
        }
        #[cfg(windows)]
        {
            self.child.stderr().take()
        }
    }
    pub fn try_wait(&mut self) -> std::io::Result<Option<std::process::ExitStatus>> {
        self.child.try_wait()
    }
    pub fn wait(&mut self) -> std::io::Result<std::process::ExitStatus> {
        self.child.wait()
    }
    pub fn terminate(&mut self, force: bool) -> std::io::Result<()> {
        #[cfg(unix)]
        {
            let signal = if force { libc::SIGKILL } else { libc::SIGTERM };
            if unsafe { libc::kill(-(self.id() as i32), signal) } < 0 {
                let e = std::io::Error::last_os_error();
                if e.raw_os_error() != Some(libc::ESRCH) {
                    return Err(e);
                }
            }
            Ok(())
        }
        #[cfg(windows)]
        {
            let _ = force;
            self.child.start_kill()
        }
    }
}
impl Drop for ProcessTree {
    fn drop(&mut self) {
        let _ = self.terminate(true);
        let _ = self.wait();
    }
}
