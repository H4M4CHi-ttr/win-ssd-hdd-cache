//! Fixed sibling-only launcher; no arbitrary executable, device or IOCTL option.
use std::process::ExitCode;
#[cfg(all(windows, target_arch = "x86_64", feature = "experimental-live"))]
fn run() -> Result<u8, ()> {
    use cache_probe_launcher::{EvidenceSink, launch_collector};
    use std::{
        io::{self, Read},
        sync::{
            Arc, Mutex,
            atomic::{AtomicU8, AtomicU64, Ordering},
        },
    };
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 2
        || args[0] != "--duration-ms"
        || args[1].starts_with('0')
        || !args[1].bytes().all(|b| b.is_ascii_digit())
    {
        return Ok(2);
    }
    let Some(duration) = args[1].parse().ok().filter(|v| (1..=30000).contains(v)) else {
        return Ok(2);
    };
    let records = EvidenceSink::new().map_err(|_| ())?;
    let started = std::time::Instant::now();
    let (child, signal) = launch_collector(duration).map_err(|_| ())?;
    let signal = Arc::new(Mutex::new(Some(signal)));
    let sender = Arc::clone(&signal);
    let status = Arc::new(AtomicU8::new(0));
    let notification_status = Arc::clone(&status);
    let requested = Arc::new(AtomicU64::new(u64::MAX));
    let request_time = Arc::clone(&requested);
    let guest_records = records.clone();
    let notification_records = records.clone();
    // Notification reader and both pipe drains run independently of synchronous
    // collector calls and each other. Binary output never shares a diagnostic pipe.
    let notification_reader =
        std::thread::Builder::new()
            .name("p04-notify".into())
            .spawn(move || {
                for byte in io::stdin().lock().bytes() {
                    match byte {
                        Ok(b'a') => {
                            let Ok(guard) = sender.lock() else { break };
                            let Some(signal) = guard.as_ref() else { break };
                            let _ = signal.publish(
                                started,
                                &request_time,
                                &notification_status,
                                &notification_records,
                            );
                        }
                        Ok(_) => {}
                        Err(_) => break,
                    }
                }
            });
    if notification_reader.is_err() {
        let mut owner = signal.lock().map_err(|_| ())?;
        if let Some(signal) = owner.as_ref() {
            let _ = signal.publish(started, &requested, &status, &records);
        }
        owner.take();
        drop(owner);
        let _ = records.finish();
        return Err(());
    }
    let result = child
        .forward(started, Arc::clone(&requested), guest_records)
        .map(|code| u8::try_from(code).unwrap_or(3))
        .map_err(|_| ());
    let mut owner = signal.lock().map_err(|_| ())?;
    if result.is_err()
        && let Some(signal) = owner.as_ref()
    {
        let _ = signal.publish(started, &requested, &status, &records);
    }
    owner.take();
    drop(owner);
    records.finish().map_err(|_| ())?;
    if status.load(Ordering::Acquire) == 2 {
        return Err(());
    }
    result
}
#[cfg(not(all(windows, target_arch = "x86_64", feature = "experimental-live")))]
fn run() -> Result<u8, ()> {
    Ok(2)
}
fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code),
        Err(()) => {
            // The diagnostic sink itself may be blocked or failed.
            ExitCode::from(3)
        }
    }
}
