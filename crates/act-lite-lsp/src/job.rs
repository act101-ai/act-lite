//! Windows job objects: a process and everything it starts, ended together.

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject, TerminateJobObject,
};
use windows_sys::Win32::System::Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME};

/// The creation flag that starts a process suspended, so it is in its job
/// before it can start anything.
pub(crate) const SUSPENDED: u32 = windows_sys::Win32::System::Threading::CREATE_SUSPENDED;

/// A job object that holds a process and every process it starts; they end
/// when it is dropped.
pub(crate) struct Job(HANDLE);

// SAFETY: a job handle may be used and closed from any thread.
unsafe impl Send for Job {}
unsafe impl Sync for Job {}

impl Job {
    /// Puts `child`, started with [`SUSPENDED`], in a new job, then lets
    /// it run. On an error `child` has not run.
    pub(crate) fn contain(child: &tokio::process::Child) -> Result<Job, String> {
        let (Some(process), Some(pid)) = (child.raw_handle(), child.id()) else {
            return Err("the process has no handle".to_string());
        };
        // SAFETY: no security attributes and no name; the handle returned
        // is owned by `Job` and closed on drop.
        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if handle.is_null() {
            return Err(format!(
                "no job object: {}",
                std::io::Error::last_os_error()
            ));
        }
        let job = Job(handle);
        // SAFETY: LIMIT_INFORMATION is plain data; the call reads exactly
        // its size from a live local.
        let limited = unsafe {
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                (&raw const limits).cast(),
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if limited == 0 {
            return Err(format!(
                "the job object could not be set to end with act-lite: {}",
                std::io::Error::last_os_error()
            ));
        }
        // SAFETY: both handles are open for the duration of the call.
        if unsafe { AssignProcessToJobObject(job.0, process as HANDLE) } == 0 {
            return Err(format!(
                "the process could not join a job object: {}",
                std::io::Error::last_os_error()
            ));
        }
        resume(pid)?;
        Ok(job)
    }

    /// Ends every process in the job.
    pub(crate) fn terminate(&self) {
        // SAFETY: the handle is open until drop.
        unsafe { TerminateJobObject(self.0, 1) };
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        // SAFETY: the handle is owned and closed exactly once.
        unsafe { CloseHandle(self.0) };
    }
}

/// Resumes the threads of process `pid`; a process started suspended has
/// one, its main thread.
fn resume(pid: u32) -> Result<(), String> {
    let failed = || {
        format!(
            "the process could not be resumed: {}",
            std::io::Error::last_os_error()
        )
    };
    // SAFETY: a snapshot of all threads; the handle is closed below.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Err(failed());
    }
    // SAFETY: THREADENTRY32 is plain data; dwSize is set as the API requires.
    let mut entry: THREADENTRY32 = unsafe { std::mem::zeroed() };
    entry.dwSize = std::mem::size_of::<THREADENTRY32>() as u32;
    let mut resumed = 0;
    // SAFETY: `snapshot` is open and `entry` is a valid THREADENTRY32.
    let mut more = unsafe { Thread32First(snapshot, &mut entry) } != 0;
    while more {
        if entry.th32OwnerProcessID == pid {
            // SAFETY: the thread handle is closed right after use.
            let thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) };
            if !thread.is_null() {
                // SAFETY: `thread` is open with THREAD_SUSPEND_RESUME.
                if unsafe { ResumeThread(thread) } != u32::MAX {
                    resumed += 1;
                }
                // SAFETY: closed exactly once.
                unsafe { CloseHandle(thread) };
            }
        }
        // SAFETY: as for Thread32First.
        more = unsafe { Thread32Next(snapshot, &mut entry) } != 0;
    }
    // SAFETY: closed exactly once.
    unsafe { CloseHandle(snapshot) };
    if resumed == 0 { Err(failed()) } else { Ok(()) }
}
