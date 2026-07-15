//! Windows Job Object: bind spawned service processes (nginx, php-cgi, DBs,
//! caches) to a kill-on-close job so they terminate together with Stackr instead
//! of lingering on their ports as orphans.

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::os::windows::io::AsRawHandle;
    use std::process::Child;
    use std::sync::OnceLock;

    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    // `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` is only accepted through the *extended*
    // limit-information structure. windows-rs gates that struct behind extra
    // features (it embeds IO_COUNTERS), so we declare the winnt.h layout directly
    // and pass it as raw bytes — keeping the dependency surface minimal.
    #[repr(C)]
    #[derive(Default)]
    struct BasicLimitInformation {
        per_process_user_time_limit: i64,
        per_job_user_time_limit: i64,
        limit_flags: u32,
        minimum_working_set_size: usize,
        maximum_working_set_size: usize,
        active_process_limit: u32,
        affinity: usize,
        priority_class: u32,
        scheduling_class: u32,
    }

    #[repr(C)]
    #[derive(Default)]
    struct IoCounters {
        read_operation_count: u64,
        write_operation_count: u64,
        other_operation_count: u64,
        read_transfer_count: u64,
        write_transfer_count: u64,
        other_transfer_count: u64,
    }

    #[repr(C)]
    #[derive(Default)]
    struct ExtendedLimitInformation {
        basic: BasicLimitInformation,
        io_info: IoCounters,
        process_memory_limit: usize,
        job_memory_limit: usize,
        peak_process_memory_used: usize,
        peak_job_memory_used: usize,
    }

    struct Job(HANDLE);
    // The handle is created once and intentionally held (never closed) for the
    // whole process lifetime. The OS closes it on exit, which — thanks to
    // JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE — terminates every assigned process.
    unsafe impl Send for Job {}
    unsafe impl Sync for Job {}

    static JOB: OnceLock<Option<Job>> = OnceLock::new();

    /// Create a kill-on-close job and return its raw handle (the caller owns it).
    pub(crate) unsafe fn create_kill_on_close_job() -> Option<HANDLE> {
        let handle = CreateJobObjectW(None, PCWSTR::null()).ok()?;
        let mut info = ExtendedLimitInformation::default();
        info.basic.limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE.0;
        SetInformationJobObject(
            handle,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const c_void,
            std::mem::size_of::<ExtendedLimitInformation>() as u32,
        )
        .ok()?;
        Some(handle)
    }

    /// Assign a child process to `job` (no-op on failure).
    pub(crate) unsafe fn assign_to(job: HANDLE, child: &Child) {
        let _ = AssignProcessToJobObject(job, HANDLE(child.as_raw_handle()));
    }

    /// Create the global job once (no-op if already created or unavailable).
    pub fn init() {
        JOB.get_or_init(|| unsafe { create_kill_on_close_job().map(Job) });
    }

    /// Bind a freshly spawned child to the job so it dies with Stackr.
    pub fn assign(child: &Child) {
        if let Some(Some(job)) = JOB.get() {
            unsafe { assign_to(job.0, child) }
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn init() {}
    pub fn assign(_child: &std::process::Child) {}
}

pub use imp::{assign, init};

#[cfg(all(test, windows))]
mod tests {
    use std::process::Command;
    use std::time::Duration;

    use windows::Win32::Foundation::CloseHandle;

    use super::imp::{assign_to, create_kill_on_close_job};

    /// Proves the mechanism: a child assigned to a kill-on-close job is
    /// terminated the moment the last job handle closes (i.e. when Stackr exits).
    #[test]
    fn kill_on_close_terminates_child() {
        unsafe {
            let job = create_kill_on_close_job().expect("create kill-on-close job");

            // A long-lived child that would linger on if not killed.
            let mut child = Command::new("ping")
                .args(["-n", "60", "127.0.0.1"])
                .spawn()
                .expect("spawn child");
            assign_to(job, &child);

            assert!(
                child.try_wait().unwrap().is_none(),
                "child should be alive before the job handle closes"
            );

            // Closing the last handle to the job must terminate the child.
            CloseHandle(job).expect("close job");
            std::thread::sleep(Duration::from_millis(1000));

            assert!(
                child.try_wait().unwrap().is_some(),
                "child must be terminated once the job handle is closed"
            );
            let _ = child.kill();
        }
    }
}
