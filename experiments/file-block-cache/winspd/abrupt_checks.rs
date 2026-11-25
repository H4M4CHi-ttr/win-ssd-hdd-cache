//! Terminate only an owned, stopped-at-a-checkpoint user-mode worker. The OS continues running.
use super::super::live::OwnedChild;
use super::*;
use std::io::{BufRead, BufReader};

const PARENT_TEST: &str = "cache::tests::winspd::recovery::abrupt::worker_loss_and_restart";
const WRITER_TEST: &str = "cache::tests::winspd::recovery::abrupt::abrupt_writer";
const WAITING: u32 = 0x50415553;

fn word(input: &mut impl Read) -> io::Result<u32> {
    let mut bytes = [0; 4];
    input.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

#[test]
#[ignore = "started by the staged WorkerLoss parent using anonymous pipes"]
fn abrupt_writer() -> io::Result<()> {
    if !exact(WRITER_TEST)? {
        return Ok(());
    }
    let mut input = BufReader::new(io::stdin().lock());
    let mut magic = [0; 4];
    input.read_exact(&mut magic)?;
    assert_eq!(&magic, b"AW01");
    let owner = word(&mut input)?;
    let mut serial = [0; 8];
    input.read_exact(&mut serial)?;
    let applied = word(&mut input)? as usize;
    assert!(matches!(applied, 0 | 512 | 1024));
    let mut c = worker_files(owner, u64::from_le_bytes(serial), 0, false)?;
    let mut out = io::stdout().lock();
    writeln!(out, "ABRUPT_WRITER_READY")?;
    out.flush()?;
    for (index, kind) in [READ, READ, WRITE].into_iter().enumerate() {
        let frame = receive(&mut input)?;
        let req = Request::decode(&frame)?;
        assert_eq!(
            (req.kind, req.hint as u32, req.first, req.count, req.fua),
            (kind, index as u32, 7, 2, false)
        );
        if kind == READ {
            let prior = c.counts;
            let result = c.sector_io(7, 2, false, req.op());
            let reply = req.response(&result)?;
            assert_eq!(reply[16], 0);
            assert!(reply[HEADER..] == original()[OFFSET..OFFSET + LENGTH]);
            let delta = (
                c.counts.origin_reads - prior.origin_reads,
                c.counts.cache_reads - prior.cache_reads,
            );
            assert_eq!(delta, if index == 0 { (2, 0) } else { (0, 2) });
            send(&mut out, &reply)?;
            continue;
        }
        assert!(req.data == patch());
        assert_eq!(
            (
                c.counts.origin_reads,
                c.counts.cache_reads,
                c.counts.origin_syncs
            ),
            (2, 2, 0)
        );
        assert_eq!(c.visible, [Some(0), Some(1)]);
        // The normal request still spans 1024 bytes. Only the lower write is controlled.
        let _unexpected = c.sector_io_with_origin(
            req.first,
            req.count,
            req.fua,
            req.op(),
            |file, bytes| {
                assert_eq!(bytes.len(), LENGTH);
                assert_eq!(file.stream_position()?, OFFSET as u64);
                if applied != 0 {
                    file.write_all(&bytes[..applied])?;
                }
                assert_eq!(file.stream_position()?, (OFFSET + applied) as u64);
                // Flush the control pipe, never the data file. Do not return to the cache machinery.
                out.write_all(&WAITING.to_le_bytes())?;
                out.write_all(&(applied as u32).to_le_bytes())?;
                out.flush()?;
                let mut wake = [0; 1];
                input.read_exact(&mut wake)?;
                Err(io::Error::other(
                    "unexpected release from the crash checkpoint",
                ))
            },
            |_| panic!("unexpected origin synchronization in crash worker"),
        );
        panic!("worker returned from a checkpoint that requires termination");
    }
    panic!("missing target Write");
}

fn start_writer(
    images: &Images,
    applied: usize,
) -> io::Result<(
    OwnedChild,
    std::process::ChildStdin,
    BufReader<std::process::ChildStdout>,
)> {
    let mut child = OwnedChild(
        Command::new(std::env::current_exe()?)
            .args(["--ignored", "--exact", WRITER_TEST, "--nocapture"])
            .current_dir(&images.dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?,
    );
    let mut input = child.0.stdin.take().unwrap();
    let mut output = BufReader::new(child.0.stdout.take().unwrap());
    let owner = std::process::id();
    let serial: u64 = images
        .dir
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .strip_prefix(&format!("win-cache-file-test-{owner}-"))
        .unwrap()
        .parse()
        .unwrap();
    input.write_all(b"AW01")?;
    input.write_all(&owner.to_le_bytes())?;
    input.write_all(&serial.to_le_bytes())?;
    input.write_all(&(applied as u32).to_le_bytes())?;
    input.flush()?;
    // libtest emits a small banner. Switch to length-prefixed binary only after the child's receipt.
    let mut banner_bytes = 0;
    loop {
        let mut line = String::new();
        let n = (&mut output).take(1024).read_line(&mut line)?;
        banner_bytes += n;
        if n == 0 || banner_bytes > 4096 {
            return Err(io::Error::other("crash worker did not start"));
        }
        if line.trim() == "ABRUPT_WRITER_READY" {
            break;
        }
    }
    Ok((child, input, output))
}

fn trial(applied: usize, reopen: &str) -> io::Result<()> {
    let old = original();
    let expected = prefix_image(applied);
    let images = Images::new(&old)?;
    let (_dir, mut origin, mut cache) = pin_files(&images)?;
    let (mut worker, mut writer_in, mut writer_out) = start_writer(&images, applied)?;
    let mut broker = start_broker("abrupt-write")?;
    let mut from_broker = broker.0.stdout.take().unwrap();
    let mut to_broker = broker.0.stdin.take().unwrap();
    for (index, kind) in [READ, READ, WRITE].into_iter().enumerate() {
        let frame = receive(&mut from_broker)?;
        let req = Request::decode(&frame)?;
        assert_eq!(
            (req.kind, req.hint as u32, req.first, req.count, req.fua),
            (kind, index as u32, 7, 2, false)
        );
        if kind == WRITE {
            assert!(req.data == patch());
        }
        send(&mut writer_in, &frame)?;
        if kind == READ {
            let reply = receive(&mut writer_out)?;
            assert_eq!(
                (
                    u64::from_le_bytes(reply[..8].try_into().unwrap()),
                    reply[8],
                    reply[16]
                ),
                (req.hint, READ, 0)
            );
            assert!(reply[HEADER..] == old[OFFSET..OFFSET + LENGTH]);
            send(&mut to_broker, &reply)?;
        }
    }
    assert_eq!(
        (word(&mut writer_out)?, word(&mut writer_out)?),
        (WAITING, applied as u32)
    );
    // Both OS process handles must still be live; the worker has not returned a Write reply.
    assert!(worker.0.try_wait()?.is_none() && broker.0.try_wait()?.is_none());
    check_file(&mut origin, &expected)?;
    check_file(&mut cache, &old[..8192])?;
    worker.0.kill()?;
    let killed = worker.0.wait()?;
    assert!(!killed.success());
    let mut extra = Vec::new();
    writer_out.read_to_end(&mut extra)?;
    assert!(
        extra.is_empty(),
        "crash worker produced a response after the checkpoint"
    );
    drop(writer_in);
    // Relay the observed EOF to the DLL callback without manufacturing a status or a payload.
    drop(to_broker);
    let status = broker.0.wait()?;
    let mut log = String::new();
    broker.0.stderr.take().unwrap().read_to_string(&mut log)?;
    from_broker.read_to_end(&mut extra)?;
    assert!(
        extra.is_empty(),
        "unexpected Rust request or shutdown-sync command"
    );
    assert_eq!(status.code(), Some(31), "unexpected broker status: {log}");
    for marker in [
        "BROKER peer-verified",
        "SDK ready inner-peer=verified dispatchers=1",
        "ABRUPT worker-eof=1 write_response_sent=0",
        "CLIENT_WRITE_TRANSPORT_ERROR lba=7 sectors=2 reaped=1 ",
        "SDK abort-close callback=1 forwarded_to_origin=0",
        "SDK stopped callbacks=3 rejected=0 close_calls=0 close_error=0 waited=1",
    ] {
        assert!(
            log.lines().filter(|l| l.starts_with(marker)).count() == 1,
            "missing or duplicated marker {marker}: {log}"
        );
    }
    assert_eq!(
        log.lines()
            .filter(|l| l.starts_with("SDK callback "))
            .count(),
        3
    );
    assert_eq!(
        log.lines()
            .filter(|l| l.starts_with("SDK outer-request "))
            .count(),
        3
    );
    check_file(&mut origin, &expected)?;
    check_file(&mut cache, &old[..8192])?;
    let diag = log
        .lines()
        .find(|l| l.starts_with("CLIENT_WRITE_TRANSPORT_ERROR "))
        .unwrap();
    println!("ABRUPT_EVENT applied_bytes={applied} {diag}");
    run_worker(&images, reopen)?;
    check_file(&mut origin, &expected)?;
    check_file(&mut cache, &expected[..8192])?;
    let sectors = match applied {
        0 => "old/old",
        512 => "new/old",
        1024 => "new/new",
        _ => unreachable!(),
    };
    println!(
        "ABRUPT_PAIR applied_bytes={applied} first_read={sectors} write_response=absent worker_killed_and_reaped=1 sdk_abort_flush_callbacks=1 loss_origin_syncs=0 broker_exit=31 reopened_origin_lines=2 reopened_cache_lines=2 reopened_shutdown_sync=1 all_bytes_verified=1"
    );
    Ok(())
}

#[test]
#[ignore = "staged WorkerLoss mode only, with the pinned official DLL and client"]
fn worker_loss_and_restart() -> io::Result<()> {
    if !exact(PARENT_TEST)? {
        return Ok(());
    }
    for (prefix, role) in [
        (512, "partial-reopen"),
        (0, "before-reopen"),
        (1024, "after-reopen"),
    ] {
        trial(prefix, role)?;
    }
    println!("FILE_CACHE_WORKER_LOSS_OK");
    Ok(())
}
