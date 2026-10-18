//! One transport's owned child tree. Never find or terminate a daemon by name.
use anyhow::{Context, Result, bail};
use std::{
    process::{Child, Command},
    time::{Duration, Instant},
};

pub(super) struct Owned {
    pub child: Child,
    confirmed_dead: bool,
    #[cfg(windows)]
    job: Job,
}

impl Owned {
    pub fn spawn(command: &mut Command) -> Result<Self> {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            use windows_sys::Win32::System::Threading::{CREATE_NO_WINDOW, CREATE_SUSPENDED};
            let job = Job::new()?;
            command
                .creation_flags(CREATE_NO_WINDOW | CREATE_SUSPENDED)
                .env("ZT_MCP_OWNED_JOB", "1");
            let child = command.spawn()?;
            let mut owned = Self {
                child,
                job,
                confirmed_dead: false,
            };
            // The suspended child cannot launch an uncontained descendant.
            owned.job.attach(&owned.child)?;
            owned.resume()?;
            Ok(owned)
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0).env("ZT_MCP_OWNED_JOB", "1");
            Ok(Self {
                child: command.spawn()?,
                confirmed_dead: false,
            })
        }
        #[cfg(not(any(windows, unix)))]
        {
            bail!("owned MCP process containment is unavailable on this platform")
        }
    }

    pub fn wait(&mut self, timeout: Duration) -> Result<bool> {
        let until = Instant::now() + timeout;
        loop {
            let exited = self.child.try_wait()?.is_some();
            #[cfg(windows)]
            let empty = self.job.active()? == 0;
            #[cfg(unix)]
            let empty = if unsafe { libc::kill(-(self.child.id() as i32), 0) } == 0 {
                false
            } else {
                let error = std::io::Error::last_os_error();
                if error.raw_os_error() != Some(libc::ESRCH) {
                    return Err(error).context("inspect owned MCP process group");
                }
                true
            };
            if exited && empty {
                self.confirmed_dead = true;
                return Ok(true);
            }
            if Instant::now() >= until {
                return Ok(false);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    pub fn terminate(&mut self, timeout: Duration) -> Result<()> {
        #[cfg(windows)]
        self.job.terminate()?;
        #[cfg(unix)]
        unsafe {
            libc::kill(-(self.child.id() as i32), libc::SIGKILL);
        }
        let _ = self.child.kill();
        if !self.wait(timeout)? {
            bail!("owned_cleanup_unconfirmed: MCP process tree did not exit");
        }
        Ok(())
    }

    #[cfg(windows)]
    fn resume(&mut self) -> Result<()> {
        use windows_sys::Win32::{
            Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
            System::{
                Diagnostics::ToolHelp::{
                    CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First,
                    Thread32Next,
                },
                Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME},
            },
        };
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(std::io::Error::last_os_error()).context("snapshot suspended MCP thread");
        }
        let mut entry = THREADENTRY32 {
            dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
            ..Default::default()
        };
        let mut found = unsafe { Thread32First(snapshot, &mut entry) } != 0;
        let mut result = Err(anyhow::anyhow!("suspended MCP thread was not found"));
        while found {
            if entry.th32OwnerProcessID == self.child.id() {
                let thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) };
                result = if thread.is_null() {
                    Err(std::io::Error::last_os_error()).context("open suspended MCP thread")
                } else {
                    let resumed = unsafe { ResumeThread(thread) };
                    let error = std::io::Error::last_os_error();
                    unsafe {
                        CloseHandle(thread);
                    }
                    if resumed == u32::MAX {
                        Err(error).context("resume owned MCP process")
                    } else {
                        Ok(())
                    }
                };
                break;
            }
            found = unsafe { Thread32Next(snapshot, &mut entry) } != 0;
        }
        unsafe {
            CloseHandle(snapshot);
        }
        result
    }
}

impl Drop for Owned {
    fn drop(&mut self) {
        if self.confirmed_dead {
            return;
        }
        // Also handles a partially initialized suspended process. No wait here:
        // explicit transport shutdown observes termination with its deadline.
        #[cfg(windows)]
        let _ = self.job.terminate();
        #[cfg(unix)]
        unsafe {
            libc::kill(-(self.child.id() as i32), libc::SIGKILL);
        }
        let _ = self.child.kill();
        let _ = self.child.try_wait();
    }
}

#[cfg(windows)]
struct Job(windows_sys::Win32::Foundation::HANDLE);

#[cfg(windows)]
impl Job {
    fn new() -> Result<Self> {
        use windows_sys::Win32::System::JobObjects::*;
        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if handle.is_null() {
            return Err(std::io::Error::last_os_error()).context("create owned MCP job");
        }
        let job = Self(handle);
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        // Only explicit independent-service launch paths request breakaway.
        limits.BasicLimitInformation.LimitFlags =
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_BREAKAWAY_OK;
        if unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const _,
                std::mem::size_of_val(&limits) as u32,
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error()).context("configure owned MCP job");
        }
        Ok(job)
    }
    fn attach(&self, child: &Child) -> Result<()> {
        use std::os::windows::io::AsRawHandle;
        if unsafe {
            windows_sys::Win32::System::JobObjects::AssignProcessToJobObject(
                self.0,
                child.as_raw_handle() as _,
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error()).context("contain suspended MCP process");
        }
        Ok(())
    }
    fn active(&self) -> Result<u32> {
        use windows_sys::Win32::System::JobObjects::*;
        let mut facts = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        if unsafe {
            QueryInformationJobObject(
                self.0,
                JobObjectBasicAccountingInformation,
                &mut facts as *mut _ as *mut _,
                std::mem::size_of_val(&facts) as u32,
                std::ptr::null_mut(),
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error()).context("inspect owned MCP job");
        }
        Ok(facts.ActiveProcesses)
    }
    fn terminate(&self) -> Result<()> {
        if unsafe { windows_sys::Win32::System::JobObjects::TerminateJobObject(self.0, 1) } == 0 {
            return Err(std::io::Error::last_os_error()).context("terminate owned MCP job");
        }
        Ok(())
    }
}

#[cfg(windows)]
impl Drop for Job {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}
