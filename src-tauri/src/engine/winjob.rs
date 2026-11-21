//! Windows Job Object wrapper: kills a whole process tree.
//!
//! `yt-dlp.exe` is a PyInstaller bootloader that starts a second Python process, which in
//! turn starts `ffmpeg.exe`. Killing only the direct child would leave the others running.
//! Every process in a Job Object (children join automatically) is terminated by
//! [`ProcessTree::kill`], and also when the handle is dropped or TubeDeck exits
//! (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`).

#[cfg(windows)]
mod imp {
    use std::{ffi::c_void, io, mem, ptr};
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            SetInformationJobObject, TerminateJobObject,
        },
    };

    pub struct ProcessTree {
        job: HANDLE,
    }

    // The job handle is an opaque kernel handle; using it from any thread is fine.
    unsafe impl Send for ProcessTree {}
    unsafe impl Sync for ProcessTree {}

    impl ProcessTree {
        /// Create a kill-on-close job and put `process` (a raw process handle) in it.
        pub fn adopt(process: *mut c_void) -> io::Result<Self> {
            unsafe {
                let job = CreateJobObjectW(ptr::null(), ptr::null());
                if job.is_null() {
                    return Err(io::Error::last_os_error());
                }
                let tree = ProcessTree { job };

                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = mem::zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let ok = SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const c_void,
                    mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                );
                if ok == 0 {
                    return Err(io::Error::last_os_error());
                }
                if AssignProcessToJobObject(job, process as HANDLE) == 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(tree)
            }
        }

        /// Terminate every process in the tree.
        pub fn kill(&self) -> io::Result<()> {
            if unsafe { TerminateJobObject(self.job, 1) } == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        }
    }

    impl Drop for ProcessTree {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.job);
            }
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use std::{ffi::c_void, io};

    /// Non-Windows stub (TubeDeck targets Windows; this keeps `cargo check` portable).
    pub struct ProcessTree;

    impl ProcessTree {
        pub fn adopt(_process: *mut c_void) -> io::Result<Self> {
            Ok(ProcessTree)
        }
        pub fn kill(&self) -> io::Result<()> {
            Ok(())
        }
    }
}

pub use imp::ProcessTree;
