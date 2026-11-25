//! Win32 ownership capsule. All pointers remain live across synchronous calls;
//! each successful fresh handle immediately acquires exactly one OwnedHandle.
//! Production child identity is pinned before CreateProcess; suspended image is
//! rechecked before ResumeThread. The synthetic build never imports device APIs.
use crate::abort_event::{ENVIRONMENT, SYNCHRONIZE};
use std::{
    ffi::c_void,
    fs::{File, OpenOptions},
    io::{self, Read},
    os::windows::{
        ffi::OsStrExt,
        fs::OpenOptionsExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle},
    },
    path::{Path, PathBuf},
    ptr,
};
#[repr(C)]
struct Security {
    length: u32,
    descriptor: *mut c_void,
    inherit: i32,
}
#[repr(C)]
struct Startup {
    size: u32,
    reserved: *mut u16,
    desktop: *mut u16,
    title: *mut u16,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    xchars: u32,
    ychars: u32,
    fill: u32,
    flags: u32,
    show: u16,
    reserved_bytes: u16,
    reserved_ptr: *mut u8,
    input: RawHandle,
    output: RawHandle,
    error: RawHandle,
}
#[repr(C)]
struct Extended {
    startup: Startup,
    attributes: *mut c_void,
}
#[repr(C)]
struct Process {
    process: RawHandle,
    thread: RawHandle,
    id: u32,
    thread_id: u32,
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateEventW(
        attributes: *const Security,
        manual: i32,
        initial: i32,
        name: *const u16,
    ) -> RawHandle;
    #[cfg(feature = "synthetic-ipc")]
    fn CreateMutexW(attributes: *const Security, owner: i32, name: *const u16) -> RawHandle;
    #[cfg(feature = "synthetic-ipc")]
    fn GetCurrentProcessId() -> u32;
    #[cfg(feature = "synthetic-ipc")]
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> RawHandle;
    fn SetEvent(handle: RawHandle) -> i32;
    fn GetCurrentProcess() -> RawHandle;
    fn DuplicateHandle(
        source_process: RawHandle,
        source: RawHandle,
        target_process: RawHandle,
        target: *mut RawHandle,
        access: u32,
        inherit: i32,
        options: u32,
    ) -> i32;
    fn CreatePipe(
        read: *mut RawHandle,
        write: *mut RawHandle,
        security: *const Security,
        size: u32,
    ) -> i32;
    fn SetHandleInformation(handle: RawHandle, mask: u32, flags: u32) -> i32;
    fn InitializeProcThreadAttributeList(
        list: *mut c_void,
        count: u32,
        flags: u32,
        size: *mut usize,
    ) -> i32;
    fn UpdateProcThreadAttribute(
        list: *mut c_void,
        flags: u32,
        attribute: usize,
        value: *mut c_void,
        size: usize,
        previous: *mut c_void,
        returned: *mut usize,
    ) -> i32;
    fn DeleteProcThreadAttributeList(list: *mut c_void);
    fn CreateProcessW(
        application: *const u16,
        command: *mut u16,
        process_security: *const c_void,
        thread_security: *const c_void,
        inherit: i32,
        flags: u32,
        environment: *const c_void,
        directory: *const u16,
        startup: *const Extended,
        process: *mut Process,
    ) -> i32;
    fn GetFileInformationByHandleEx(
        file: RawHandle,
        class: u32,
        information: *mut c_void,
        bytes: u32,
    ) -> i32;
    fn QueryFullProcessImageNameW(
        process: RawHandle,
        flags: u32,
        name: *mut u16,
        length: *mut u32,
    ) -> i32;
    fn ResumeThread(thread: RawHandle) -> u32;
    fn TerminateProcess(process: RawHandle, exit: u32) -> i32;
    fn WaitForSingleObject(handle: RawHandle, milliseconds: u32) -> u32;
    fn GetExitCodeProcess(process: RawHandle, exit: *mut u32) -> i32;
    #[cfg(feature = "synthetic-ipc")]
    fn GetHandleInformation(handle: RawHandle, flags: *mut u32) -> i32;
}
#[link(name = "bcrypt")]
unsafe extern "system" {
    fn BCryptGenRandom(algorithm: *mut c_void, buffer: *mut u8, length: u32, flags: u32) -> i32;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Trial([u8; 16]);
#[derive(Clone, Copy)]
pub enum RecordKind {
    RequestPublished,
    NotificationFailed,
    DeadlineUnconfirmed,
    ChildExit,
    ReceivedEof,
    ForwardingComplete,
    MonitorLost,
}
impl Trial {
    fn fresh() -> io::Result<Self> {
        let mut bytes = [0; 16];
        if unsafe { BCryptGenRandom(ptr::null_mut(), bytes.as_mut_ptr(), 16, 2) } < 0 {
            return Err(io::Error::other("trial randomness unavailable"));
        }
        Ok(Self(bytes))
    }
    pub fn evidence(
        self,
        kind: RecordKind,
        at_ms: u64,
        state: Option<crate::supervision::Snapshot>,
    ) -> String {
        let id: String = self.0.iter().map(|byte| format!("{byte:02x}")).collect();
        let event = match kind {
            RecordKind::RequestPublished => "request_published",
            RecordKind::NotificationFailed => "notification_failed",
            RecordKind::DeadlineUnconfirmed => "deadline_unconfirmed",
            RecordKind::ChildExit => "child_exit_observed",
            RecordKind::ReceivedEof => "stdout_eof_observed",
            RecordKind::ForwardingComplete => "stdout_forwarded",
            RecordKind::MonitorLost => "monitor_lost",
        };
        fn number(value: Option<u64>) -> String {
            value.map_or_else(|| "null".to_owned(), |value| value.to_string())
        }
        fn boolean(value: Option<bool>) -> &'static str {
            match value {
                Some(true) => "true",
                Some(false) => "false",
                None => "null",
            }
        }
        let request = state.and_then(|value| value.request_published_at).or(
            if matches!(kind, RecordKind::RequestPublished) {
                Some(at_ms)
            } else {
                None
            },
        );
        let received_hash = state
            .and_then(|value| value.facts.received_hash)
            .map_or_else(
                || "null".to_owned(),
                |value| {
                    format!(
                        "\"{}\"",
                        value
                            .iter()
                            .map(|byte| format!("{byte:02x}"))
                            .collect::<String>()
                    )
                },
            );
        format!(
            "{{\"schema\":\"p04_guest_supervision_v1\",\"trial_id\":\"{id}\",\"event\":\"{event}\",\"at_ms\":{at_ms},\"request_published_at_ms\":{},\"child_exit_observed\":{},\"child_exit_at_ms\":{},\"stdout_eof_observed\":{},\"stdout_eof_at_ms\":{},\"received_bytes\":{},\"received_sha256\":{received_hash},\"receiver_hash_authenticated\":null,\"stdout_forwarding_complete\":{},\"saved\":{},\"deadline_unconfirmed_at_ms\":{},\"monitor_lost_at_ms\":{}}}",
            number(request),
            boolean(state.map(|value| value.facts.exited)),
            number(state.and_then(|value| value.process_exited_at)),
            boolean(state.map(|value| value.facts.stdout_eof)),
            number(state.and_then(|value| value.output_complete_at)),
            number(state.map(|value| value.facts.received_bytes)),
            boolean(state.map(|value| value.facts.forwarding_complete)),
            boolean(state.and_then(|value| value.facts.saved)),
            number(state.and_then(|value| value.deadline_unconfirmed_at)),
            number(state.and_then(|value| value.monitor_lost_at))
        )
    }
}
/// Bounded diagnostic transport. Sink backpressure never blocks notifications,
/// receive hashing, or the independent guest watcher's clock.
#[derive(Clone)]
pub struct EvidenceSink {
    sender: std::sync::mpsc::SyncSender<String>,
    issued: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    written: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    failed: std::sync::Arc<std::sync::atomic::AtomicBool>,
}
impl EvidenceSink {
    pub fn new() -> io::Result<Self> {
        use std::io::Write;
        Self::with_record_writer(|line| {
            let mut output = io::stderr().lock();
            output.write_all(line)?;
            output.flush()
        })
    }
    fn with_record_writer(
        mut write_record: impl FnMut(&[u8]) -> io::Result<()> + Send + 'static,
    ) -> io::Result<Self> {
        use std::sync::{
            Arc,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        };
        let (sender, receiver) = std::sync::mpsc::sync_channel::<String>(16);
        let value = Self {
            sender,
            issued: Arc::new(AtomicUsize::new(0)),
            written: Arc::new(AtomicUsize::new(0)),
            failed: Arc::new(AtomicBool::new(false)),
        };
        let written = Arc::clone(&value.written);
        let failed = Arc::clone(&value.failed);
        std::thread::Builder::new()
            .name("p04-evidence".into())
            .spawn(move || {
                while let Ok(mut record) = receiver.recv() {
                    record.push('\n');
                    if write_record(record.as_bytes()).is_err() {
                        failed.store(true, Ordering::Release);
                        break;
                    }
                    written.fetch_add(1, Ordering::Release);
                }
            })?;
        Ok(value)
    }
    fn mark_failed(&self) {
        self.failed
            .store(true, std::sync::atomic::Ordering::Release);
    }
    pub fn try_send(&self, record: String) -> io::Result<()> {
        use std::sync::atomic::Ordering;
        if self.sender.try_send(record).is_err() {
            self.failed.store(true, Ordering::Release);
            return Err(io::Error::other("evidence queue unavailable"));
        }
        self.issued.fetch_add(1, Ordering::Release);
        Ok(())
    }
    pub fn finish(&self) -> io::Result<()> {
        use std::sync::atomic::Ordering;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
        loop {
            if self.failed.load(Ordering::Acquire) {
                return Err(io::Error::other("evidence unavailable"));
            }
            if self.written.load(Ordering::Acquire) >= self.issued.load(Ordering::Acquire) {
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                return Err(io::Error::other("evidence flush unconfirmed"));
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}
fn own(raw: RawHandle) -> io::Result<OwnedHandle> {
    if raw.is_null() || raw == (-1isize) as RawHandle {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: only freshly returned valid owning Win32 results enter this function.
    Ok(unsafe { OwnedHandle::from_raw_handle(raw) })
}
fn duplicate(raw: RawHandle, access: u32, inherit: bool) -> io::Result<OwnedHandle> {
    let mut copy = ptr::null_mut();
    // SAFETY: borrowed live source, current process pseudo is never inherited.
    if unsafe {
        DuplicateHandle(
            GetCurrentProcess(),
            raw,
            GetCurrentProcess(),
            &mut copy,
            access,
            i32::from(inherit),
            0,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    own(copy)
}
fn event() -> io::Result<OwnedHandle> {
    // SAFETY: null name creates fresh unnamed event; null security means noninherit.
    own(unsafe { CreateEventW(ptr::null(), 1, 0, ptr::null()) })
}
fn pipe() -> io::Result<(File, File)> {
    let security = Security {
        length: size_of::<Security>() as u32,
        descriptor: ptr::null_mut(),
        inherit: 1,
    };
    let (mut read, mut write) = (ptr::null_mut(), ptr::null_mut());
    if unsafe { CreatePipe(&mut read, &mut write, &security, 0) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let read = own(read)?;
    let write = own(write)?;
    Ok((File::from(read), File::from(write)))
}
fn noninherit(handle: RawHandle) -> io::Result<()> {
    if unsafe { SetHandleInformation(handle, 1, 0) } == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
struct Attributes {
    storage: Vec<usize>,
    initialized: bool,
}
impl Attributes {
    fn new(handles: &mut [RawHandle]) -> io::Result<Self> {
        let mut bytes = 0;
        unsafe {
            InitializeProcThreadAttributeList(ptr::null_mut(), 1, 0, &mut bytes);
        }
        if bytes == 0 || bytes > 65536 {
            return Err(io::Error::other("attribute size unavailable"));
        }
        let mut value = Self {
            storage: vec![0; bytes.div_ceil(size_of::<usize>())],
            initialized: false,
        };
        if unsafe { InitializeProcThreadAttributeList(value.pointer(), 1, 0, &mut bytes) } == 0 {
            return Err(io::Error::last_os_error());
        }
        value.initialized = true;
        if unsafe {
            UpdateProcThreadAttribute(
                value.pointer(),
                0,
                0x0002_0002,
                handles.as_mut_ptr().cast(),
                size_of_val(handles),
                ptr::null_mut(),
                ptr::null_mut(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(value)
    }
    fn pointer(&mut self) -> *mut c_void {
        self.storage.as_mut_ptr().cast()
    }
}
impl Drop for Attributes {
    fn drop(&mut self) {
        if self.initialized {
            unsafe { DeleteProcThreadAttributeList(self.pointer()) }
        }
    }
}
pub struct Signal {
    trial: Trial,
    handle: OwnedHandle,
}
impl Signal {
    pub fn trial(&self) -> Trial {
        self.trial
    }
    #[cfg(feature = "synthetic-ipc")]
    pub fn synthetic_raw(&self) -> RawHandle {
        self.handle.as_raw_handle()
    }
    pub fn publish(
        &self,
        started: std::time::Instant,
        requested: &std::sync::atomic::AtomicU64,
        status: &std::sync::atomic::AtomicU8,
        records: &EvidenceSink,
    ) -> io::Result<()> {
        use std::sync::atomic::Ordering;
        let outcome = if self.notify().is_ok() { 1 } else { 2 };
        let at = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        if outcome == 1 {
            let _ = requested.compare_exchange(u64::MAX, at, Ordering::AcqRel, Ordering::Acquire);
        }
        let previous = status.fetch_max(outcome, Ordering::AcqRel);
        if outcome > previous {
            let _ = records.try_send(self.trial.evidence(
                if outcome == 1 {
                    RecordKind::RequestPublished
                } else {
                    RecordKind::NotificationFailed
                },
                at,
                None,
            ));
        }
        if outcome == 1 {
            Ok(())
        } else {
            Err(io::Error::other("notification unavailable"))
        }
    }
    pub fn notify(&self) -> io::Result<()> {
        if unsafe { SetEvent(self.handle.as_raw_handle()) } == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}
pub struct Child {
    trial: Trial,
    process: OwnedHandle,
    // Holds write/delete exclusion on exact trusted image through child lifetime.
    _image: File,
    pub input: File,
    pub output: File,
    pub diagnostics: File,
}
impl Child {
    pub fn trial(&self) -> Trial {
        self.trial
    }
    pub fn forward(
        self,
        started: std::time::Instant,
        requested: std::sync::Arc<std::sync::atomic::AtomicU64>,
        records: EvidenceSink,
    ) -> io::Result<u32> {
        let diagnostic_records = records.clone();
        let trial = self.trial;
        let watcher = GuestWatch::start(&self.process, self.trial, started, requested, records)?;
        let observed = std::sync::Arc::clone(&watcher.observed);
        let Child {
            trial: _,
            process,
            _image,
            input,
            output,
            diagnostics,
        } = self;
        drop(input);
        let out = std::thread::Builder::new()
            .name("p04-binary-drain".into())
            .spawn(move || copy_stream_observed(output, io::stdout().lock(), Some(observed)))?;
        let unavailable_records = diagnostic_records.clone();
        let err = std::thread::Builder::new()
            .name("p04-diagnostic-drain".into())
            .spawn(move || {
                drain_child_diagnostics(diagnostics, trial, started, &diagnostic_records)
            });
        let err = match err {
            Ok(reader) => reader,
            Err(error) => {
                return finish_partial_reader_start(
                    error,
                    out,
                    trial,
                    started,
                    &unavailable_records,
                );
            }
        };
        let process_result = (|| {
            if unsafe { WaitForSingleObject(process.as_raw_handle(), u32::MAX) } != 0 {
                return Err(io::Error::last_os_error());
            }
            let mut code = 0;
            if unsafe { GetExitCodeProcess(process.as_raw_handle(), &mut code) } == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(code)
        })();
        finish_forwarding(process_result, out, err)
    }
    pub fn wait(&self, milliseconds: u32) -> io::Result<Option<u32>> {
        match unsafe { WaitForSingleObject(self.process.as_raw_handle(), milliseconds) } {
            258 => Ok(None),
            0 => {
                let mut code = 0;
                if unsafe { GetExitCodeProcess(self.process.as_raw_handle(), &mut code) } == 0 {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(Some(code))
                }
            }
            _ => Err(io::Error::last_os_error()),
        }
    }
    #[cfg(feature = "synthetic-ipc")]
    pub fn terminate_synthetic(&self) -> io::Result<()> {
        if unsafe { TerminateProcess(self.process.as_raw_handle(), 99) } == 0 {
            Err(io::Error::last_os_error())
        } else {
            self.wait(5000)?
                .ok_or_else(|| io::Error::other("synthetic child did not exit"))
                .map(|_| ())
        }
    }
}
fn finish_partial_reader_start(
    error: io::Error,
    out: std::thread::JoinHandle<io::Result<u64>>,
    trial: Trial,
    started: std::time::Instant,
    records: &EvidenceSink,
) -> io::Result<u32> {
    records.mark_failed();
    let at = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let _ = records.try_send(trial.child_evidence(
        "diagnostic_failure",
        at,
        serde_json::json!({"code":"DIAGNOSTIC_READER_UNAVAILABLE","complete":false}),
    ));
    // The second producer never started. Keep the watcher/notify paths alive
    // until the already-started producer has returned; never detach it on error.
    let _ = out.join();
    Err(error)
}
fn finish_forwarding(
    process_result: io::Result<u32>,
    out: std::thread::JoinHandle<io::Result<u64>>,
    diagnostics: std::thread::JoinHandle<io::Result<u64>>,
) -> io::Result<u32> {
    // Join both producers before any result can escape to evidence.finish().
    let output_result = out
        .join()
        .map_err(|_| io::Error::other("output reader panicked"))
        .and_then(|result| result);
    let diagnostic_result = diagnostics
        .join()
        .map_err(|_| io::Error::other("diagnostic reader panicked"))
        .and_then(|result| result);
    let code = process_result?;
    output_result?;
    diagnostic_result?;
    Ok(code)
}
fn image(path: &Path) -> io::Result<File> {
    OpenOptions::new().read(true).share_mode(1).open(path)
}
fn same_file(left: &File, right: &File) -> io::Result<bool> {
    #[repr(C)]
    #[derive(Default, PartialEq, Eq)]
    struct Identity {
        volume: u64,
        id: [u8; 16],
    }
    fn identify(file: &File) -> io::Result<Identity> {
        let mut value = Identity::default();
        // SAFETY: FileIdInfo class18 writes exactly the initialized fixed struct.
        if unsafe {
            GetFileInformationByHandleEx(
                file.as_raw_handle(),
                18,
                (&mut value as *mut Identity).cast(),
                size_of::<Identity>() as u32,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(value)
    }
    Ok(identify(left)? == identify(right)?)
}
#[cfg(feature = "experimental-live")]
fn sibling(name: &str) -> io::Result<PathBuf> {
    let executable = std::env::current_exe()?.canonicalize()?;
    let directory = executable
        .parent()
        .ok_or_else(|| io::Error::other("no sibling directory"))?;
    let actual = directory.join(name).canonicalize()?;
    if actual.parent() != Some(directory) || actual.file_name() != Some(std::ffi::OsStr::new(name))
    {
        return Err(io::Error::other(
            "sibling identity escaped trusted directory",
        ));
    }
    Ok(actual)
}
fn spawn(
    trial: Trial,
    path: PathBuf,
    argument: &str,
    event_handle: RawHandle,
    extra_env: &[(&str, String)],
) -> io::Result<Child> {
    // Only fixed sibling names and fixed/strictly numeric argument tokens reach here.
    let pinned = image(&path)?;
    let (child_input, input) = pipe()?;
    let (output, child_output) = pipe()?;
    let (diagnostics, child_error) = pipe()?;
    #[cfg(feature = "synthetic-ipc")]
    let child_output = if argument == "no-pipe" {
        let null = OpenOptions::new().write(true).open("NUL")?;
        let mut raw = ptr::null_mut();
        if unsafe {
            DuplicateHandle(
                GetCurrentProcess(),
                null.as_raw_handle(),
                GetCurrentProcess(),
                &mut raw,
                0,
                1,
                2,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        File::from(own(raw)?)
    } else {
        child_output
    };
    noninherit(input.as_raw_handle())?;
    noninherit(output.as_raw_handle())?;
    noninherit(diagnostics.as_raw_handle())?;
    let mut handles = [
        child_input.as_raw_handle(),
        child_output.as_raw_handle(),
        child_error.as_raw_handle(),
        event_handle,
    ];
    let mut attributes = Attributes::new(&mut handles)?;
    let mut startup: Extended = unsafe { std::mem::zeroed() };
    startup.startup.size = size_of::<Extended>() as u32;
    startup.startup.flags = 0x100;
    startup.startup.input = child_input.as_raw_handle();
    startup.startup.output = child_output.as_raw_handle();
    startup.startup.error = child_error.as_raw_handle();
    startup.attributes = attributes.pointer();
    let application: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    if application[..application.len() - 1].contains(&0)
        || path.as_os_str().to_string_lossy().contains('"')
    {
        return Err(io::Error::other("invalid image path"));
    }
    let mut command: Vec<u16> = format!("\"{}\" {argument}", path.display())
        .encode_utf16()
        .chain(Some(0))
        .collect();
    // Exact child environment: never copy the parent's reserved or general entries.
    let mut entries = vec![(ENVIRONMENT, format!("{:016x}", event_handle as usize))];
    entries.extend_from_slice(extra_env);
    entries.sort_by_key(|(key, _)| key.to_ascii_uppercase());
    let mut environment = Vec::new();
    for (key, value) in entries {
        environment.extend(format!("{key}={value}").encode_utf16());
        environment.push(0);
    }
    environment.push(0);
    let mut info: Process = unsafe { std::mem::zeroed() };
    // SAFETY: all buffers/list and inheritable handles live through CreateProcess.
    // Suspended child cannot execute unverified image; only allowlisted handles pass.
    if unsafe {
        CreateProcessW(
            application.as_ptr(),
            command.as_mut_ptr(),
            ptr::null(),
            ptr::null(),
            1,
            0x0008_0000 | 0x400 | 4 | 0x0800_0000,
            environment.as_ptr().cast(),
            ptr::null(),
            &startup,
            &mut info,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let process = own(info.process)?;
    let thread = own(info.thread)?;
    let checked = (|| {
        let mut actual = vec![0u16; 32768];
        let mut len = actual.len() as u32;
        if unsafe {
            QueryFullProcessImageNameW(process.as_raw_handle(), 0, actual.as_mut_ptr(), &mut len)
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        actual.truncate(len as usize);
        use std::os::windows::ffi::OsStringExt;
        let actual = PathBuf::from(std::ffi::OsString::from_wide(&actual));
        if !same_file(&pinned, &image(&actual)?)? {
            return Err(io::Error::other("child image identity mismatch"));
        }
        if unsafe { ResumeThread(thread.as_raw_handle()) } == u32::MAX {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    })();
    if let Err(error) = checked {
        // Never resume a mismatched/suspended image. This is startup cleanup only.
        unsafe {
            TerminateProcess(process.as_raw_handle(), 3);
            WaitForSingleObject(process.as_raw_handle(), 5000);
        }
        return Err(error);
    }
    Ok(Child {
        trial,
        process,
        _image: pinned,
        input,
        output,
        diagnostics,
    })
}
#[cfg(feature = "experimental-live")]
pub fn launch_collector(duration: u32) -> io::Result<(Child, Signal)> {
    if !(1..=30000).contains(&duration) {
        return Err(io::Error::other("invalid duration"));
    }
    let trial = Trial::fresh()?;
    let parent = event()?;
    let child = duplicate(parent.as_raw_handle(), SYNCHRONIZE, true)?;
    let process = spawn(
        trial,
        sibling("cache-probe-collector.exe")?,
        &format!("collect-controlled --experimental-live --duration-ms {duration}"),
        child.as_raw_handle(),
        &[],
    )?;
    Ok((
        process,
        Signal {
            trial,
            handle: parent,
        },
    ))
}
#[cfg(feature = "synthetic-ipc")]
#[derive(Clone, Copy)]
pub enum Synthetic {
    Normal,
    ExcessRights,
    WrongType,
    Stale,
    Missing,
    Zero,
    InvalidDigits,
    Width,
    Pseudo,
    NoPipe,
    SenderExit,
    NonInheritableDecoy,
}
#[cfg(feature = "synthetic-ipc")]
pub fn launch_synthetic(kind: Synthetic) -> io::Result<(Child, Signal)> {
    let trial = Trial::fresh()?;
    let parent = event()?;
    let child = match kind {
        Synthetic::ExcessRights => duplicate(parent.as_raw_handle(), SYNCHRONIZE | 2, true)?,
        Synthetic::WrongType => {
            let security = Security {
                length: size_of::<Security>() as u32,
                descriptor: ptr::null_mut(),
                inherit: 1,
            };
            let mutex = own(unsafe { CreateMutexW(&security, 0, ptr::null()) })?;
            duplicate(mutex.as_raw_handle(), SYNCHRONIZE, true)?
        }
        _ => duplicate(parent.as_raw_handle(), SYNCHRONIZE, true)?,
    };
    // Inheritable sentinel intentionally excluded from STARTUPINFOEX allowlist.
    let sentinel = duplicate(parent.as_raw_handle(), SYNCHRONIZE | 2, true)?;
    let extras = [
        (
            "P04_TEST_SUPERVISOR",
            unsafe { GetCurrentProcessId() }.to_string(),
        ),
        (
            "P04_TEST_SENTINEL",
            format!("{:016x}", sentinel.as_raw_handle() as usize),
        ),
        (
            "P04_TEST_PARENT",
            format!("{:016x}", parent.as_raw_handle() as usize),
        ),
    ];
    let argument = match kind {
        Synthetic::Normal => "normal",
        Synthetic::ExcessRights => "excess",
        Synthetic::WrongType => "wrong-type",
        Synthetic::Stale => "stale",
        Synthetic::Missing => "missing",
        Synthetic::Zero => "zero",
        Synthetic::InvalidDigits => "digits",
        Synthetic::Width => "width",
        Synthetic::Pseudo => "pseudo",
        Synthetic::NoPipe => "no-pipe",
        Synthetic::SenderExit => "sender-exit",
        Synthetic::NonInheritableDecoy => "non-inheritable",
    };
    let exe = std::env::current_exe()?;
    let directory = exe
        .parent()
        .ok_or_else(|| io::Error::other("missing executable directory"))?;
    let directory = if directory.file_name().is_some_and(|name| name == "deps") {
        directory
            .parent()
            .ok_or_else(|| io::Error::other("missing target directory"))?
    } else {
        directory
    };
    let process = spawn(
        trial,
        directory.join("p04-ipc-worker.exe").canonicalize()?,
        argument,
        child.as_raw_handle(),
        &extras,
    )?;
    Ok((
        process,
        Signal {
            trial,
            handle: parent,
        },
    ))
}
#[cfg(feature = "synthetic-ipc")]
// The OS treats a HANDLE as an opaque value; no Rust pointee is accessed.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn synthetic_mutation_denied(handle: RawHandle) -> bool {
    unsafe { SetEvent(handle) == 0 }
}
#[cfg(feature = "synthetic-ipc")]
// The OS treats a HANDLE as an opaque value; no Rust pointee is accessed.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn synthetic_not_inherited(handle: RawHandle) -> bool {
    crate::abort_event::invalid_handle_for_synthetic_check(handle)
}
#[cfg(feature = "synthetic-ipc")]
// The OS treats a HANDLE as an opaque value; no Rust pointee is accessed.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn synthetic_inheritance_cleared(handle: RawHandle) -> bool {
    let mut flags = 1;
    unsafe { GetHandleInformation(handle, &mut flags) != 0 && flags & 1 == 0 }
}
#[cfg(feature = "synthetic-ipc")]
/// # Safety
/// Only a fresh, valid, exclusively owned synthetic startup handle may be passed.
pub unsafe fn synthetic_close_raw(handle: RawHandle) {
    drop(unsafe { OwnedHandle::from_raw_handle(handle) })
}
const MAX_CHILD_LINE: usize = 4096;
const MAX_CHILD_BYTES: u64 = 65536;
const MAX_CHILD_RECORDS: u64 = 4;
const CONTROLLED_KEYS: [&str; 14] = [
    "schema",
    "first_reason",
    "reasons",
    "abort_accepted",
    "begin_submitted",
    "begin_acknowledged",
    "stop_acknowledged",
    "terminal_received",
    "send_result",
    "host_deadline",
    "process_exit",
    "received_bytes",
    "received_hash",
    "saved",
];
const STOP_REASONS: [&str; 7] = [
    "UNCERTAIN",
    "CONTROL_FAILURE",
    "PRODUCER",
    "EXTERNAL_ABORT",
    "LIMIT",
    "DURATION",
    "TERMINAL_TIMEOUT",
];
const COLLECTOR_CODES: [&str; 10] = [
    "PROBE_USAGE",
    "PROBE_LIVE_DISABLED",
    "PROBE_STDOUT_PIPE_REQUIRED",
    "PROBE_ABORT_CONTROL_INVALID",
    "PROBE_ABORT_CONTROL_IO",
    "PROBE_RNG_UNAVAILABLE",
    "PROBE_DEVICE_UNAVAILABLE",
    "PROBE_OUTPUT_IO",
    "INVALID_PROBE_ARCHIVE",
    "ABORT_BEFORE_BEGIN",
];
fn child_record(line: &[u8]) -> Option<(&'static str, serde_json::Value)> {
    let text = std::str::from_utf8(line).ok()?;
    if COLLECTOR_CODES.contains(&text) {
        return Some(("collector_error", serde_json::json!({"code":text})));
    }
    let value: serde_json::Value = serde_json::from_slice(line).ok()?;
    let object = value.as_object()?;
    if object.len() != CONTROLLED_KEYS.len()
        || CONTROLLED_KEYS.iter().any(|key| !object.contains_key(*key))
        || object["schema"] != "p04-controlled-v1"
    {
        return None;
    }
    // Allowed strings cannot contain colons: extra key delimiters detect duplicate
    // keys that a generic JSON map would otherwise silently overwrite.
    if line.iter().filter(|byte| **byte == b':').count() != CONTROLLED_KEYS.len() {
        return None;
    }
    if !object["first_reason"].is_null()
        && !STOP_REASONS.contains(&object["first_reason"].as_str()?)
    {
        return None;
    }
    let reasons = object["reasons"].as_array()?;
    if reasons.len() > STOP_REASONS.len() {
        return None;
    }
    let mut reason_mask = 0u8;
    for reason in reasons {
        let bit = STOP_REASONS
            .iter()
            .position(|allowed| Some(*allowed) == reason.as_str())?;
        if reason_mask & (1 << bit) != 0 {
            return None;
        }
        reason_mask |= 1 << bit;
    }
    for key in ["abort_accepted", "begin_submitted"] {
        if !object[key].is_boolean() {
            return None;
        }
    }
    for key in [
        "begin_acknowledged",
        "stop_acknowledged",
        "terminal_received",
    ] {
        if !object[key].is_null() && !object[key].is_boolean() {
            return None;
        }
    }
    for key in [
        "send_result",
        "host_deadline",
        "process_exit",
        "received_bytes",
        "received_hash",
        "saved",
    ] {
        if !object[key].is_null() {
            return None;
        }
    }
    Some(("controlled_report", value))
}
impl Trial {
    fn child_evidence(self, event: &'static str, at_ms: u64, payload: serde_json::Value) -> String {
        let trial_id: String = self.0.iter().map(|byte| format!("{byte:02x}")).collect();
        serde_json::json!({"schema":"p04_child_diagnostic_v1","trial_id":trial_id,"event":event,"at_ms":at_ms,"payload":payload}).to_string()
    }
}
fn drain_child_diagnostics(
    mut source: impl Read,
    trial: Trial,
    started: std::time::Instant,
    records: &EvidenceSink,
) -> io::Result<u64> {
    let now = || u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let fail = |code: &'static str| {
        records.mark_failed();
        let _ = records.try_send(trial.child_evidence(
            "diagnostic_failure",
            now(),
            serde_json::json!({"code":code,"complete":false}),
        ));
    };
    let (mut total, mut count) = (0u64, 0u64);
    let mut pending = Vec::new();
    let mut failure = false;
    let mut bytes = [0; 1024];
    loop {
        let n = match source.read(&mut bytes) {
            Ok(n) => n,
            Err(_) => {
                if !failure {
                    fail("DIAGNOSTIC_READ_FAILED")
                }
                return Err(io::Error::other("child diagnostic read failed"));
            }
        };
        if n == 0 {
            break;
        }
        total = total.saturating_add(n as u64);
        if failure {
            continue;
        }
        if total > MAX_CHILD_BYTES {
            fail("DIAGNOSTIC_BYTE_LIMIT");
            failure = true;
            continue;
        }
        for byte in &bytes[..n] {
            if *byte == b'\n' {
                if pending.last() == Some(&b'\r') {
                    pending.pop();
                }
                count += 1;
                let parsed = if count <= MAX_CHILD_RECORDS {
                    child_record(&pending)
                } else {
                    None
                };
                match parsed {
                    Some((event, payload)) => {
                        if records
                            .try_send(trial.child_evidence(event, now(), payload))
                            .is_err()
                        {
                            failure = true;
                            break;
                        }
                    }
                    None => {
                        fail(if count > MAX_CHILD_RECORDS {
                            "DIAGNOSTIC_RECORD_LIMIT"
                        } else {
                            "DIAGNOSTIC_INVALID_RECORD"
                        });
                        failure = true;
                        break;
                    }
                }
                pending.clear();
            } else if pending.len() < MAX_CHILD_LINE {
                pending.push(*byte)
            } else {
                fail("DIAGNOSTIC_LINE_LIMIT");
                failure = true;
                break;
            }
        }
    }
    if !failure && !pending.is_empty() {
        fail("DIAGNOSTIC_TRUNCATED");
        failure = true;
    }
    if failure {
        return Err(io::Error::other("child diagnostics incomplete"));
    }
    records.try_send(trial.child_evidence(
        "diagnostic_eof",
        now(),
        serde_json::json!({"complete":true,"records":count,"received_bytes":total}),
    ))?;
    Ok(total)
}

fn copy_stream_observed(
    mut source: File,
    mut destination: impl io::Write,
    observed: Option<std::sync::Arc<std::sync::Mutex<crate::supervision::ProcessObservation>>>,
) -> io::Result<u64> {
    use sha2::{Digest, Sha256};
    let mut received_hash = Sha256::new();
    let mut total = 0;
    let mut failure = None;
    let mut bytes = [0; 8192];
    loop {
        let n = source.read(&mut bytes)?;
        if n == 0 {
            break;
        }
        total += n as u64;
        received_hash.update(&bytes[..n]);
        if let Some(ref observed) = observed {
            observed
                .lock()
                .map_err(|_| io::Error::other("monitor lock poisoned"))?
                .received_bytes = total;
        }
        if failure.is_none()
            && let Err(error) = destination.write_all(&bytes[..n])
        {
            failure = Some(error)
        }
    }
    if let Some(ref observed) = observed {
        let mut facts = observed
            .lock()
            .map_err(|_| io::Error::other("monitor lock poisoned"))?;
        facts.stdout_eof = true;
        facts.received_hash = Some(received_hash.finalize().into());
    }
    if let Some(error) = failure {
        return Err(error);
    }
    destination.flush()?;
    if let Some(ref observed) = observed {
        observed
            .lock()
            .map_err(|_| io::Error::other("monitor lock poisoned"))?
            .forwarding_complete = true;
    }
    Ok(total)
}

struct GuestWatch {
    observed: std::sync::Arc<std::sync::Mutex<crate::supervision::ProcessObservation>>,
    finished: std::sync::Arc<std::sync::atomic::AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl GuestWatch {
    fn start(
        process: &OwnedHandle,
        trial: Trial,
        started: std::time::Instant,
        requested: std::sync::Arc<std::sync::atomic::AtomicU64>,
        records: EvidenceSink,
    ) -> io::Result<Self> {
        use crate::supervision::{Monitor, ProcessObservation};
        use std::sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
        };
        let process = duplicate(process.as_raw_handle(), SYNCHRONIZE | 0x1000, false)?;
        let observed = Arc::new(Mutex::new(ProcessObservation::default()));
        let finished = Arc::new(AtomicBool::new(false));
        let facts = Arc::clone(&observed);
        let stop = Arc::clone(&finished);
        let thread = std::thread::Builder::new()
            .name("p04-guest-watch".into())
            .spawn(move || {
                let mut monitor = Monitor::new(15000).expect("fixed nonzero guest deadline");
                let mut previous = monitor.snapshot();
                loop {
                    let published = requested.load(Ordering::Acquire);
                    let now = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
                    if published != u64::MAX
                        && monitor.snapshot().request_published_at.is_none()
                        && monitor.note_request_published(published, now).is_err()
                    {
                        let _ = monitor.lose_monitor(now);
                    }
                    let mut observation = match facts.lock() {
                        Ok(value) => *value,
                        Err(_) => {
                            let _ = monitor.lose_monitor(now);
                            let _ = records.try_send(trial.evidence(
                                RecordKind::MonitorLost,
                                now,
                                Some(monitor.snapshot()),
                            ));
                            break;
                        }
                    };
                    match unsafe { WaitForSingleObject(process.as_raw_handle(), 0) } {
                        0 => observation.exited = true,
                        258 => {}
                        _ => {
                            let _ = monitor.lose_monitor(now);
                        }
                    }
                    if monitor.observe(now, observation).is_err() {
                        let _ = monitor.lose_monitor(now);
                    }
                    let current = monitor.snapshot();
                    for (label, before, after) in [
                        (
                            RecordKind::DeadlineUnconfirmed,
                            previous.deadline_unconfirmed_at,
                            current.deadline_unconfirmed_at,
                        ),
                        (
                            RecordKind::ChildExit,
                            previous.process_exited_at,
                            current.process_exited_at,
                        ),
                        (
                            RecordKind::ReceivedEof,
                            previous.output_complete_at,
                            current.output_complete_at,
                        ),
                        (
                            RecordKind::MonitorLost,
                            previous.monitor_lost_at,
                            current.monitor_lost_at,
                        ),
                    ] {
                        if before.is_none()
                            && let Some(at) = after
                        {
                            let _ = records.try_send(trial.evidence(label, at, Some(current)));
                        }
                    }
                    if !previous.facts.forwarding_complete && current.facts.forwarding_complete {
                        let _ = records.try_send(trial.evidence(
                            RecordKind::ForwardingComplete,
                            now,
                            Some(current),
                        ));
                    }
                    previous = current;
                    if stop.load(Ordering::Acquire) {
                        break;
                    }
                    std::thread::park_timeout(std::time::Duration::from_secs(1));
                }
            })?;
        Ok(Self {
            observed,
            finished,
            thread: Some(thread),
        })
    }
}
impl Drop for GuestWatch {
    fn drop(&mut self) {
        self.finished
            .store(true, std::sync::atomic::Ordering::Release);
        if let Some(thread) = self.thread.take() {
            thread.thread().unpark();
            let _ = thread.join();
        }
    }
}
#[cfg(feature = "synthetic-ipc")]
pub fn export_synthetic_child(child: Child, supervisor: u32) -> io::Result<Vec<u8>> {
    use std::io::Write;
    let target = own(unsafe { OpenProcess(0x40, 0, supervisor) })?;
    let sources = [
        (child.process.as_raw_handle(), 0x00101001),
        (child.input.as_raw_handle(), 0),
        (child.output.as_raw_handle(), 0),
        (child.diagnostics.as_raw_handle(), 0),
    ];
    let mut remote: Vec<RawHandle> = Vec::new();
    for (source, access) in sources {
        let mut copy = ptr::null_mut();
        let success = unsafe {
            DuplicateHandle(
                GetCurrentProcess(),
                source,
                target.as_raw_handle(),
                &mut copy,
                access,
                0,
                if access == 0 { 2 } else { 0 },
            )
        };
        if success == 0 {
            let failure = io::Error::last_os_error();
            for raw in remote {
                let mut local = ptr::null_mut();
                if unsafe {
                    DuplicateHandle(
                        target.as_raw_handle(),
                        raw,
                        GetCurrentProcess(),
                        &mut local,
                        0,
                        0,
                        3,
                    )
                } != 0
                {
                    drop(own(local)?);
                }
            }
            return Err(failure);
        }
        remote.push(copy);
    }
    // Private binary control pipe only; never print handle values or save them
    // as diagnostics. Values are fresh handles already assigned in supervisor.
    let mut packet = Vec::with_capacity(48);
    packet.extend_from_slice(&child.trial.0);
    for raw in remote {
        packet.write_all(&(raw as usize as u64).to_le_bytes())?;
    }
    Ok(packet)
}
#[cfg(feature = "synthetic-ipc")]
/// # Safety
/// Packet must be the exact private output of the trusted synthetic sender,
/// addressed to this process. Each transferred handle is adopted only once.
pub unsafe fn import_synthetic_child(packet: &[u8]) -> io::Result<Child> {
    if packet.len() != 48 {
        return Err(io::Error::other("invalid synthetic transfer"));
    }
    let trial = Trial(
        packet[..16]
            .try_into()
            .map_err(|_| io::Error::other("invalid trial tag"))?,
    );
    let mut raw = [ptr::null_mut(); 4];
    for (index, bytes) in packet[16..].as_chunks::<8>().0.iter().enumerate() {
        let value = u64::from_le_bytes(*bytes);
        if value == 0 || value > isize::MAX as u64 {
            return Err(io::Error::other("invalid transfer handle"));
        }
        raw[index] = value as usize as RawHandle;
    }
    // SAFETY: caller establishes exclusive transferred ownership, never ambient
    // input. Adopt all handles before any fallible filesystem operation.
    let process = own(raw[0])?;
    let input = File::from(own(raw[1])?);
    let output = File::from(own(raw[2])?);
    let diagnostics = File::from(own(raw[3])?);
    let executable = std::env::current_exe()?.canonicalize()?;
    let _image = image(&executable)?;
    Ok(Child {
        trial,
        process,
        _image,
        input,
        output,
        diagnostics,
    })
}

#[cfg(feature = "synthetic-ipc")]
pub fn synthetic_noninheritable_decoy() -> io::Result<OwnedHandle> {
    let source = event()?;
    duplicate(source.as_raw_handle(), SYNCHRONIZE, false)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn c03_guest_evidence_preserves_trial_actual_publication_and_unknown_save() {
        use crate::supervision::{Monitor, ProcessObservation};
        let trial = Trial([0x5a; 16]);
        let issued = trial.evidence(RecordKind::RequestPublished, 100, None);
        assert!(issued.contains("\"trial_id\":\"5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a\""));
        assert!(issued.contains("\"request_published_at_ms\":100"));
        for field in [
            "child_exit_observed",
            "stdout_eof_observed",
            "received_bytes",
            "saved",
        ] {
            assert!(issued.contains(&format!("\"{field}\":null")));
        }
        let mut monitor = Monitor::new(15000).unwrap();
        monitor
            .observe(14999, ProcessObservation::default())
            .unwrap();
        monitor.note_request_published(100, 14999).unwrap();
        monitor
            .observe(15000, ProcessObservation::default())
            .unwrap();
        monitor
            .observe(
                16000,
                ProcessObservation {
                    exited: true,
                    stdout_eof: true,
                    received_bytes: 7,
                    received_hash: Some([42; 32]),
                    forwarding_complete: true,
                    saved: None,
                },
            )
            .unwrap();
        let late = trial.evidence(
            RecordKind::ForwardingComplete,
            16000,
            Some(monitor.snapshot()),
        );
        for expected in [
            "\"request_published_at_ms\":100",
            "\"deadline_unconfirmed_at_ms\":15000",
            "\"child_exit_at_ms\":16000",
            "\"received_bytes\":7",
            "\"received_sha256\":\"2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a\"",
            "\"stdout_forwarding_complete\":true",
            "\"saved\":null",
        ] {
            assert!(late.contains(expected), "missing {expected}; actual {late}");
        }
        assert!(late.contains("\"receiver_hash_authenticated\":null"));
    }
    #[test]
    fn b03_receiver_sha256_known_vectors_and_multiple_chunks() {
        use std::{
            io::Write,
            sync::{Arc, Mutex},
        };
        for (input, expected) in [
            (
                b"abc".to_vec(),
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            ),
            (
                vec![b'a'; 20000],
                "cc17faaad36649c4603dda4d8ff97cb149722af0bcac0746305a2134ad2d0b97",
            ),
        ] {
            let (read, mut write) = pipe().unwrap();
            let sent = input.clone();
            let producer = std::thread::spawn(move || write.write_all(&sent).unwrap());
            let observed = Arc::new(Mutex::new(crate::supervision::ProcessObservation::default()));
            let mut output = Vec::new();
            assert_eq!(
                copy_stream_observed(read, &mut output, Some(Arc::clone(&observed))).unwrap(),
                input.len() as u64
            );
            producer.join().unwrap();
            assert_eq!(output, input);
            let facts = *observed.lock().unwrap();
            let measured: String = facts
                .received_hash
                .unwrap()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            assert_eq!(measured, expected);
            assert!(facts.stdout_eof && facts.forwarding_complete);
            assert_eq!(facts.saved, None);
        }
    }
    #[test]
    fn b03_receiver_flush_failure_keeps_received_hash_without_forward_or_save() {
        use std::{
            io::Write,
            sync::{Arc, Mutex},
        };
        struct FailsFlush;
        impl Write for FailsFlush {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Err(io::Error::other("synthetic flush failure"))
            }
        }
        let (read, mut write) = pipe().unwrap();
        let producer = std::thread::spawn(move || write.write_all(b"abc").unwrap());
        let observed = Arc::new(Mutex::new(crate::supervision::ProcessObservation::default()));
        assert!(copy_stream_observed(read, FailsFlush, Some(Arc::clone(&observed))).is_err());
        producer.join().unwrap();
        let facts = *observed.lock().unwrap();
        assert_eq!(facts.received_bytes, 3);
        assert!(facts.stdout_eof && facts.received_hash.is_some());
        assert!(!facts.forwarding_complete);
        assert_eq!(facts.saved, None);
    }
    fn isolated_evidence(capacity: usize) -> (EvidenceSink, std::sync::mpsc::Receiver<String>) {
        use std::sync::{
            Arc,
            atomic::{AtomicBool, AtomicUsize},
        };
        let (sender, receiver) = std::sync::mpsc::sync_channel(capacity);
        (
            EvidenceSink {
                sender,
                issued: Arc::new(AtomicUsize::new(0)),
                written: Arc::new(AtomicUsize::new(0)),
                failed: Arc::new(AtomicBool::new(false)),
            },
            receiver,
        )
    }
    #[test]
    fn c03_publish_uses_actual_time_and_records_once_outside_owner() {
        use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
        let signal = Signal {
            trial: Trial([0x17; 16]),
            handle: event().unwrap(),
        };
        let child = duplicate(signal.handle.as_raw_handle(), SYNCHRONIZE, false).unwrap();
        let requested = AtomicU64::new(u64::MAX);
        let status = AtomicU8::new(0);
        let (records, received) = isolated_evidence(16);
        assert_eq!(
            unsafe { WaitForSingleObject(child.as_raw_handle(), 0) },
            258
        );
        let started = std::time::Instant::now();
        signal
            .publish(started, &requested, &status, &records)
            .unwrap();
        signal
            .publish(started, &requested, &status, &records)
            .unwrap();
        assert_eq!(unsafe { WaitForSingleObject(child.as_raw_handle(), 0) }, 0);
        let actual = requested.load(Ordering::Acquire);
        assert_ne!(actual, u64::MAX);
        assert_eq!(status.load(Ordering::Acquire), 1);
        let record = received.try_recv().unwrap();
        assert!(record.contains(&format!("\"request_published_at_ms\":{actual}")));
        assert!(record.contains("\"child_exit_observed\":null"));
        assert!(received.try_recv().is_err());
    }
    #[test]
    fn b03_evidence_backpressure_is_nonblocking_and_cannot_report_flush_success() {
        let (records, _receiver) = isolated_evidence(1);
        records.try_send("synthetic first".to_owned()).unwrap();
        assert!(records.try_send("synthetic second".to_owned()).is_err());
        assert!(records.finish().is_err());
    }
    fn child_report_fixture() -> Vec<u8> {
        serde_json::json!({"schema":"p04-controlled-v1","first_reason":"EXTERNAL_ABORT","reasons":["EXTERNAL_ABORT"],"abort_accepted":true,"begin_submitted":true,"begin_acknowledged":true,"stop_acknowledged":true,"terminal_received":true,"send_result":null,"host_deadline":null,"process_exit":null,"received_bytes":null,"received_hash":null,"saved":null}).to_string().into_bytes()
    }
    struct Fragments {
        bytes: std::io::Cursor<Vec<u8>>,
        width: usize,
    }
    impl Read for Fragments {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            let count = buffer.len().min(self.width);
            self.bytes.read(&mut buffer[..count])
        }
    }
    #[test]
    fn b03_child_diagnostics_fragment_interleave_and_final_eof_are_json() {
        use std::sync::{Arc, Mutex};
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let output = Arc::clone(&bytes);
        let records = EvidenceSink::with_record_writer(move |line| {
            // Force partial destination writes/yields while other producers run.
            for byte in line {
                output.lock().unwrap().push(*byte);
                std::thread::yield_now();
            }
            Ok(())
        })
        .unwrap();
        let trial = Trial([0x3c; 16]);
        let mut source = child_report_fixture();
        source.extend_from_slice(b"\nPROBE_OUTPUT_IO\n");
        let expected_bytes = source.len();
        let child_records = records.clone();
        let child = std::thread::spawn(move || {
            drain_child_diagnostics(
                Fragments {
                    bytes: std::io::Cursor::new(source),
                    width: 1,
                },
                trial,
                std::time::Instant::now(),
                &child_records,
            )
        });
        let guest_records = records.clone();
        let guest = std::thread::spawn(move || {
            for at in 0..3 {
                guest_records
                    .try_send(trial.evidence(RecordKind::RequestPublished, at, None))
                    .unwrap();
            }
        });
        assert_eq!(child.join().unwrap().unwrap(), expected_bytes as u64);
        guest.join().unwrap();
        records.finish().unwrap();
        let output = String::from_utf8(bytes.lock().unwrap().clone()).unwrap();
        assert!(output.ends_with('\n'));
        let parsed: Vec<serde_json::Value> = output
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(parsed.len(), 6);
        assert!(
            parsed
                .iter()
                .all(|value| value["trial_id"] == "3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c")
        );
        let report = parsed
            .iter()
            .find(|value| value["event"] == "controlled_report")
            .unwrap();
        assert_eq!(report["payload"]["schema"], "p04-controlled-v1");
        assert_eq!(report["payload"]["saved"], serde_json::Value::Null);
        let eof = parsed
            .iter()
            .find(|value| value["event"] == "diagnostic_eof")
            .unwrap();
        assert_eq!(eof["payload"]["complete"], true);
        assert_eq!(eof["payload"]["records"], 2);
        assert_eq!(eof["payload"]["received_bytes"], expected_bytes);
    }
    #[test]
    fn b03_child_diagnostics_reject_truncation_overflow_and_private_text() {
        let mut duplicate = child_report_fixture();
        duplicate.splice(1..1, b"\"saved\":null,".iter().copied());
        duplicate.push(b'\n');
        let mut unknown: serde_json::Value =
            serde_json::from_slice(&child_report_fixture()).unwrap();
        unknown["source_path"] = serde_json::json!("PRIVATE_SOURCE_PATH_MARKER");
        let mut partial = child_report_fixture();
        partial.extend_from_slice(b"\n{\"schema\":");
        for source in [
            child_report_fixture(),
            partial,
            vec![b'x'; MAX_CHILD_LINE + 1],
            b"PRIVATE_SOURCE_PATH_MARKER\n".to_vec(),
            [unknown.to_string().as_bytes(), b"\n"].concat(),
            duplicate,
            vec![255, b'\n'],
            b"PROBE_OUTPUT_IO\n".repeat(5),
        ] {
            let (records, received) = isolated_evidence(16);
            assert!(
                drain_child_diagnostics(
                    Fragments {
                        bytes: std::io::Cursor::new(source),
                        width: 3
                    },
                    Trial([0x3c; 16]),
                    std::time::Instant::now(),
                    &records
                )
                .is_err()
            );
            assert!(records.finish().is_err());
            let lines: Vec<String> = received.try_iter().collect();
            assert!(
                lines
                    .iter()
                    .all(|line| !line.contains("PRIVATE_SOURCE_PATH_MARKER"))
            );
            let parsed: Vec<serde_json::Value> = lines
                .iter()
                .map(|line| serde_json::from_str(line).unwrap())
                .collect();
            assert!(
                parsed
                    .iter()
                    .any(|value| value["event"] == "diagnostic_failure"
                        && value["payload"]["complete"] == false)
            );
            assert!(
                parsed
                    .iter()
                    .all(|value| value["event"] != "diagnostic_eof")
            );
        }
        struct FailsAtEof(std::io::Cursor<Vec<u8>>);
        impl Read for FailsAtEof {
            fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
                let n = self.0.read(buffer)?;
                if n == 0 {
                    Err(io::Error::other("PRIVATE_SOURCE_PATH_MARKER"))
                } else {
                    Ok(n)
                }
            }
        }
        let (records, received) = isolated_evidence(16);
        assert!(
            drain_child_diagnostics(
                FailsAtEof(std::io::Cursor::new(b"PROBE_OUTPUT_IO\n".to_vec())),
                Trial([0x3c; 16]),
                std::time::Instant::now(),
                &records
            )
            .is_err()
        );
        let output: Vec<String> = received.try_iter().collect();
        assert!(records.finish().is_err());
        assert!(
            output
                .iter()
                .all(|line| !line.contains("PRIVATE_SOURCE_PATH_MARKER"))
        );
        assert!(
            output
                .iter()
                .any(|line| line.contains("DIAGNOSTIC_READ_FAILED"))
        );
        assert!(output.iter().all(|line| !line.contains("diagnostic_eof")));
    }
    #[test]
    fn b03_child_diagnostics_backpressure_and_sink_failure_are_not_complete() {
        let (records, received) = isolated_evidence(1);
        let trial = Trial([0x3c; 16]);
        records
            .try_send(trial.evidence(RecordKind::RequestPublished, 0, None))
            .unwrap();
        let sender = records.clone();
        let (done, finished) = std::sync::mpsc::channel();
        let mut input = b"PROBE_OUTPUT_IO\n".to_vec();
        input.extend(vec![b'x'; 20000]);
        let expected = input.len() as u64;
        let drain = std::thread::spawn(move || {
            let mut source = std::io::Cursor::new(input);
            let result =
                drain_child_diagnostics(&mut source, trial, std::time::Instant::now(), &sender);
            done.send((result, source.position())).unwrap();
        });
        let result = finished.recv_timeout(std::time::Duration::from_secs(5));
        drop(received);
        drain.join().unwrap();
        let (result, consumed) = result.expect("bounded child diagnostic drain");
        assert!(result.is_err());
        assert_eq!(consumed, expected);
        assert!(records.finish().is_err());
        let bytes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let partial = std::sync::Arc::clone(&bytes);
        let sink = EvidenceSink::with_record_writer(move |line| {
            partial.lock().unwrap().extend_from_slice(&line[..5]);
            Err(io::Error::other("synthetic sink failure"))
        })
        .unwrap();
        sink.try_send(trial.evidence(RecordKind::RequestPublished, 0, None))
            .unwrap();
        assert!(sink.finish().is_err());
        assert_eq!(sink.written.load(std::sync::atomic::Ordering::Acquire), 0);
        assert!(!bytes.lock().unwrap().contains(&b'\n'));
    }
    #[test]
    fn b03_binary_forward_failure_waits_for_final_diagnostic_and_evidence_flush() {
        use std::{
            io::Write,
            sync::{Arc, Mutex},
            time::Duration,
        };
        for failure in ["binary_destination", "process_wait", "process_exit_query"] {
            let bytes = Arc::new(Mutex::new(Vec::new()));
            let output = Arc::clone(&bytes);
            let records = EvidenceSink::with_record_writer(move |line| {
                output.lock().unwrap().extend_from_slice(line);
                Ok(())
            })
            .unwrap();
            let trial = Trial([0x61; 16]);
            let (read, mut write) = pipe().unwrap();
            let producer =
                std::thread::spawn(move || write.write_all(&[0, 255, 13, 10, 80, 48, 52]).unwrap());
            let observed = Arc::new(Mutex::new(crate::supervision::ProcessObservation::default()));
            let facts = Arc::clone(&observed);
            let (output_done, output_ready) = std::sync::mpsc::channel();
            struct Destination {
                fails: bool,
            }
            impl Write for Destination {
                fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                    if self.fails {
                        Err(io::Error::other("synthetic binary destination failure"))
                    } else {
                        Ok(bytes.len())
                    }
                }
                fn flush(&mut self) -> io::Result<()> {
                    Ok(())
                }
            }
            let out = std::thread::spawn(move || {
                let result = copy_stream_observed(
                    read,
                    Destination {
                        fails: failure == "binary_destination",
                    },
                    Some(facts),
                );
                output_done.send(()).unwrap();
                result
            });
            let (entered, entry) = std::sync::mpsc::channel();
            let (release, barrier) = std::sync::mpsc::channel();
            let diagnostic_records = records.clone();
            let diagnostics = std::thread::spawn(move || {
                entered.send(()).unwrap();
                barrier
                    .recv_timeout(Duration::from_secs(5))
                    .map_err(|_| io::Error::other("synthetic diagnostic release missing"))?;
                let mut report = child_report_fixture();
                report.push(b'\n');
                drain_child_diagnostics(
                    std::io::Cursor::new(report),
                    trial,
                    std::time::Instant::now(),
                    &diagnostic_records,
                )
            });
            let (done, finished) = std::sync::mpsc::channel();
            let completion_records = records.clone();
            let completion = std::thread::spawn(move || {
                let process_result = if failure == "binary_destination" {
                    Ok(3)
                } else {
                    Err(io::Error::other(failure))
                };
                let result = finish_forwarding(process_result, out, diagnostics);
                let flushed = completion_records.finish();
                done.send((result, flushed)).unwrap();
            });
            output_ready.recv_timeout(Duration::from_secs(5)).unwrap();
            entry.recv_timeout(Duration::from_secs(5)).unwrap();
            // The diagnostic producer is definitely blocked outside the joiner.
            // Cleanup releases it even if a regression already returned early.
            let early = finished.recv_timeout(Duration::from_millis(100));
            let returned_early = early.is_ok();
            let _ = release.send(());
            let (result, flushed) = match early {
                Ok(value) => value,
                Err(_) => finished
                    .recv_timeout(Duration::from_secs(5))
                    .expect("bounded forwarding completion"),
            };
            producer.join().unwrap();
            completion.join().unwrap();
            assert!(
                !returned_early,
                "forwarding result escaped before final diagnostic for {failure}"
            );
            assert!(result.is_err());
            assert!(flushed.is_ok());
            let output = String::from_utf8(bytes.lock().unwrap().clone()).unwrap();
            let values: Vec<serde_json::Value> = output
                .lines()
                .map(|line| serde_json::from_str(line).unwrap())
                .collect();
            assert_eq!(values.len(), 2);
            assert!(
                values
                    .iter()
                    .all(|value| value["trial_id"] == "61616161616161616161616161616161")
            );
            assert!(
                values
                    .iter()
                    .any(|value| value["event"] == "controlled_report")
            );
            assert!(
                values.iter().any(|value| value["event"] == "diagnostic_eof"
                    && value["payload"]["complete"] == true)
            );
            let facts = *observed.lock().unwrap();
            assert_eq!(facts.received_bytes, 7);
            assert!(facts.stdout_eof && facts.received_hash.is_some());
            assert_eq!(facts.forwarding_complete, failure != "binary_destination");
            assert_eq!(facts.saved, None);
        }
        // Deterministic second-spawn error: the first reader is still pending.
        let trial = Trial([0x62; 16]);
        let (read, mut write) = pipe().unwrap();
        write.write_all(b"abc").unwrap();
        drop(write);
        let (entered, entry) = std::sync::mpsc::channel();
        let (release, barrier) = std::sync::mpsc::channel();
        let observed = Arc::new(Mutex::new(crate::supervision::ProcessObservation::default()));
        let facts = Arc::clone(&observed);
        let out = std::thread::spawn(move || {
            entered.send(()).unwrap();
            barrier
                .recv_timeout(Duration::from_secs(5))
                .map_err(|_| io::Error::other("synthetic binary reader release missing"))?;
            copy_stream_observed(read, Vec::new(), Some(facts))
        });
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let output = Arc::clone(&bytes);
        let (record_seen, seen) = std::sync::mpsc::channel();
        let records = EvidenceSink::with_record_writer(move |line| {
            output.lock().unwrap().extend_from_slice(line);
            record_seen.send(()).unwrap();
            Ok(())
        })
        .unwrap();
        let completion_records = records.clone();
        let (done, finished) = std::sync::mpsc::channel();
        let completion = std::thread::spawn(move || {
            let result = finish_partial_reader_start(
                io::Error::other("synthetic second-reader spawn failed"),
                out,
                trial,
                std::time::Instant::now(),
                &completion_records,
            );
            done.send((result, completion_records.finish())).unwrap();
        });
        entry.recv_timeout(Duration::from_secs(5)).unwrap();
        seen.recv_timeout(Duration::from_secs(5)).unwrap();
        let early = finished.recv_timeout(Duration::from_millis(100));
        let returned_early = early.is_ok();
        let _ = release.send(());
        let (result, flushed) = match early {
            Ok(value) => value,
            Err(_) => finished
                .recv_timeout(Duration::from_secs(5))
                .expect("bounded partial-start cleanup"),
        };
        completion.join().unwrap();
        assert!(
            !returned_early,
            "second-spawn error detached the binary reader"
        );
        assert_eq!(
            result.unwrap_err().to_string(),
            "synthetic second-reader spawn failed"
        );
        assert!(flushed.is_err());
        let output = String::from_utf8(bytes.lock().unwrap().clone()).unwrap();
        let parsed: Vec<serde_json::Value> = output
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0]["trial_id"], "62626262626262626262626262626262");
        assert_eq!(parsed[0]["event"], "diagnostic_failure");
        assert_eq!(
            parsed[0]["payload"]["code"],
            "DIAGNOSTIC_READER_UNAVAILABLE"
        );
        assert_eq!(parsed[0]["payload"]["complete"], false);
        let facts = *observed.lock().unwrap();
        assert_eq!(facts.received_bytes, 3);
        assert!(facts.stdout_eof && facts.forwarding_complete);
        assert_eq!(facts.saved, None);
    }
}
