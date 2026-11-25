//! This binary has no device transport dependency or registration entry point.
#[cfg(all(windows, target_arch = "x86_64"))]
fn run() -> Result<(), ()> {
    use cache_probe_launcher::{
        abort_event::{AbortEvent, ENVIRONMENT, EventError, parse, stdout_is_pipe},
        synthetic_close_raw, synthetic_inheritance_cleared, synthetic_mutation_denied,
        synthetic_not_inherited,
    };
    use std::{
        io::{self, Read, Write},
        os::windows::io::{AsRawHandle, RawHandle},
    };
    let mode = std::env::args().nth(1).ok_or(())?;
    let original = std::env::var(ENVIRONMENT).map_err(|_| ())?;
    if std::env::vars_os().any(|(key, _)| {
        key != ENVIRONMENT
            && key != "P04_TEST_SENTINEL"
            && key != "P04_TEST_PARENT"
            && key != "P04_TEST_SUPERVISOR"
    }) {
        return Err(());
    }
    if mode == "sender-exit" {
        let supervisor = std::env::var("P04_TEST_SUPERVISOR")
            .map_err(|_| ())?
            .parse::<u32>()
            .map_err(|_| ())?;
        let (child, signal) =
            cache_probe_launcher::launch_synthetic(cache_probe_launcher::Synthetic::Normal)
                .map_err(|_| ())?;
        let packet =
            cache_probe_launcher::export_synthetic_child(child, supervisor).map_err(|_| ())?;
        drop(signal);
        io::stdout().write_all(&packet).map_err(|_| ())?;
        io::stdout().flush().map_err(|_| ())?;
        eprintln!("SENDER_EXITING");
        return Ok(());
    }
    // Same type/rights, but this local owner must survive rejected ambient input.
    let decoy = if mode == "non-inheritable" {
        let handle = cache_probe_launcher::synthetic_noninheritable_decoy().map_err(|_| ())?;
        cache_probe_launcher::abort_event::synthetic_event_properties(&handle).map_err(|_| ())?;
        unsafe {
            std::env::set_var(
                ENVIRONMENT,
                format!("{:016x}", handle.as_raw_handle() as usize),
            )
        };
        Some(handle)
    } else {
        None
    };
    // Synthetic-only single-thread child can replace its own startup fixture.
    match mode.as_str() {
        "missing" => unsafe { std::env::remove_var(ENVIRONMENT) },
        "zero" => unsafe { std::env::set_var(ENVIRONMENT, "0000000000000000") },
        "digits" => unsafe { std::env::set_var(ENVIRONMENT, "000000000000000G") },
        "width" => unsafe { std::env::set_var(ENVIRONMENT, "10000000000000000") },
        "pseudo" => unsafe { std::env::set_var(ENVIRONMENT, "ffffffffffffffff") },
        "stale" => unsafe { synthetic_close_raw(parse(&original).map_err(|_| ())? as RawHandle) },
        "normal" | "excess" | "wrong-type" | "no-pipe" | "non-inheritable" => {}
        _ => return Err(()),
    }
    if mode == "no-pipe" {
        if stdout_is_pipe() {
            return Err(());
        }
        eprintln!("REFUSED");
        return Ok(());
    }
    if !stdout_is_pipe() {
        return Err(());
    }
    let before = cache_probe_launcher::abort_event::synthetic_ownership_observations();
    // SAFETY: fixed launcher transfers valid startup fixtures exclusively. The
    // owned local non-inheritable decoy must fail before adoption. If a regression
    // accepts it, relinquish the prior owner before dropping the accepted result.
    let accepted = unsafe { AbortEvent::from_environment() };
    if let Some(decoy) = decoy {
        let raw = decoy.as_raw_handle();
        match accepted {
            Err(EventError::Invalid) => {
                if cache_probe_launcher::abort_event::synthetic_ownership_observations() != before
                    || !synthetic_inheritance_cleared(raw)
                {
                    return Err(());
                }
                drop(decoy);
                if !synthetic_not_inherited(raw) {
                    return Err(());
                }
                eprintln!("REFUSED");
                return Ok(());
            }
            Ok(event) => {
                std::mem::forget(decoy);
                drop(event);
                return Err(());
            }
            Err(_) => return Err(()),
        }
    }
    if mode != "normal" {
        if !matches!(accepted, Err(EventError::Invalid)) {
            return Err(());
        }
        eprintln!("REFUSED");
        return Ok(());
    }
    let mut abort = accepted.map_err(|_| ())?;
    let raw = abort.raw_for_synthetic_check();
    if !synthetic_mutation_denied(raw) || !synthetic_inheritance_cleared(raw) {
        return Err(());
    }
    for key in ["P04_TEST_SENTINEL", "P04_TEST_PARENT"] {
        let raw = parse(&std::env::var(key).map_err(|_| ())?).map_err(|_| ())? as RawHandle;
        let excluded = cache_probe_launcher::abort_event::excluded_alias_for_synthetic_check(
            raw,
            abort.raw_for_synthetic_check(),
        );
        if !excluded {
            return Err(());
        }
    }
    if abort.poll().map_err(|_| ())? {
        return Err(());
    }
    eprintln!("READY");
    // Real blocking pipe barrier: event notification cannot unblock this read.
    let mut byte = [0];
    io::stdin().read_exact(&mut byte).map_err(|_| ())?;
    let wanted = match byte[0] {
        b'A' => true,
        b'N' => false,
        _ => return Err(()),
    };
    if abort.poll().map_err(|_| ())? != wanted || abort.poll().map_err(|_| ())? != wanted {
        return Err(());
    }
    eprintln!("{}", if wanted { "ACCEPTED" } else { "NOT_NOTIFIED" });
    drop(abort);
    // The same raw value is now closed. No allocation occurs before verification.
    if !synthetic_not_inherited(raw) {
        return Err(());
    }
    io::stdout()
        .write_all(&[0, 255, 13, 10, 80, 48, 52])
        .map_err(|_| ())?;
    io::stdout().flush().map_err(|_| ())?;
    eprintln!("CLOSED");
    Ok(())
}
#[cfg(not(all(windows, target_arch = "x86_64")))]
fn run() -> Result<(), ()> {
    Err(())
}
fn main() -> std::process::ExitCode {
    if run().is_ok() {
        std::process::ExitCode::SUCCESS
    } else {
        eprintln!("P04_SYNTHETIC_FAILURE");
        std::process::ExitCode::from(3)
    }
}
