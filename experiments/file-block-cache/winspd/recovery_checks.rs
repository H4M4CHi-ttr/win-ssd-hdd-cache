//! Exercise two indistinguishable client failures using separate file-backed Rust workers.
use super::live::{receive, send, start_broker};
use super::*;
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::process::{Command, Stdio};

const PARENT: &str = "cache::tests::winspd::recovery::restart_after_lost_write";
const WORKER: &str = "cache::tests::winspd::recovery::recovery_worker";
const OFFSET: usize = 7 * 512;
const LENGTH: usize = 2 * 512;

fn exact(name: &str) -> io::Result<bool> {
    if !std::env::args()
        .skip(1)
        .eq(["--ignored", "--exact", name, "--nocapture"])
    {
        return Ok(false);
    }
    assert_eq!(
        std::env::current_exe()?.file_name().unwrap(),
        "file-cache-pipe-test.exe"
    );
    Ok(true)
}

fn original() -> Vec<u8> {
    (0..131 * 512)
        .map(|i| ((i / 512 * 31 + i % 239) % 251) as u8)
        .collect()
}

fn patch() -> Vec<u8> {
    // Known v1.0B1 x64 write-pattern words for LBA 7 and 8, independently fixed for this fixture.
    [0x46ab_cca5_93a3_c687_u64, 0x9120_9a1f_f7f4_f1d5]
        .into_iter()
        .flat_map(|word| word.to_le_bytes().repeat(64))
        .collect()
}

fn image(applied: bool) -> Vec<u8> {
    prefix_image(if applied { LENGTH } else { 0 })
}

fn prefix_image(prefix: usize) -> Vec<u8> {
    let mut bytes = original();
    bytes[OFFSET..OFFSET + prefix].copy_from_slice(&patch()[..prefix]);
    bytes
}

fn check_file(file: &mut File, expected: &[u8]) -> io::Result<()> {
    let meta = file.metadata()?;
    assert!(meta.is_file() && meta.file_attributes() & 0x400 == 0);
    assert_eq!(meta.len(), expected.len() as u64);
    file.rewind()?;
    let mut bytes = vec![0; expected.len()];
    file.read_exact(&mut bytes)?;
    assert!(
        bytes == expected,
        "fixture contents differ from the independent image"
    );
    Ok(())
}

// The orchestrating test exclusively creates the directory and both files and holds
// handles denying delete/rename until both workers finish. Workers never accept paths.
fn worker_files(
    owner: u32,
    serial: u64,
    prefix_len: usize,
    recover: bool,
) -> io::Result<FileCache> {
    let cwd = std::env::current_dir()?;
    let exe = std::env::current_exe()?;
    let fixtures = exe.parent().unwrap().join("fixtures");
    assert_eq!(cwd.parent(), Some(fixtures.as_path()));
    let basename = cwd.file_name().unwrap().to_str().unwrap();
    assert_eq!(basename, format!("win-cache-file-test-{owner}-{serial}"));
    let stage = exe.parent().unwrap().file_name().unwrap().to_str().unwrap();
    let nonce = stage
        .strip_prefix("file-cache-winspd-")
        .expect("unexpected test staging directory");
    assert!(nonce.len() == 32 && nonce.bytes().all(|b| b.is_ascii_hexdigit()));
    for path in [
        &fixtures,
        &cwd,
        &cwd.join("origin.img"),
        &cwd.join("cache.img"),
    ] {
        assert_eq!(
            fs::symlink_metadata(path)?.file_attributes() & 0x400,
            0,
            "unexpected reparse point"
        );
    }
    let open = |leaf| {
        OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(3)
            .custom_flags(0x0020_0000)
            .open(cwd.join(leaf))
    };
    let mut origin = open("origin.img")?;
    let mut cache = open("cache.img")?;
    check_file(
        &mut origin,
        &prefix_image(if recover { prefix_len } else { 0 }),
    )?;
    if recover {
        check_file(&mut cache, &original()[..8192])?;
    } else {
        check_file(&mut cache, &[0; 8192])?;
    }
    FileCache::new(origin, cache)
}

fn exercise_worker(owner: u32, serial: u64, name: &str) -> io::Result<()> {
    let (prefix_len, recover) = match name {
        "before-loss" => (0, false),
        "after-loss" => (LENGTH, false),
        "before-reopen" => (0, true),
        "partial-reopen" => (512, true),
        "after-reopen" => (LENGTH, true),
        _ => panic!("unknown worker role"),
    };
    let applied = prefix_len == LENGTH;
    let mut c = worker_files(owner, serial, prefix_len, recover)?;
    let read_image = prefix_image(if recover { prefix_len } else { 0 });
    let mode = if recover {
        "recover-read"
    } else if applied {
        "lost-after"
    } else {
        "lost-before"
    };
    let mut broker = start_broker(mode)?;
    let mut input = broker.0.stdout.take().unwrap();
    let mut output = broker.0.stdin.take().unwrap();
    let mut writes = 0;
    let sequence = if !recover && applied {
        &[READ, READ, WRITE][..]
    } else {
        &[READ, READ][..]
    };
    for (i, &op) in sequence.iter().enumerate() {
        let msg = receive(&mut input)?;
        let r = Request::decode(&msg)?;
        assert_eq!(
            (r.hint as u32, r.kind, r.first, r.count, r.fua),
            (i as u32, op, 7, 2, false)
        );
        c.origin.seek(SeekFrom::Start(1))?;
        c.cache.seek(SeekFrom::Start(1))?;
        let prior = c.counts;
        if op == WRITE {
            assert!(r.data == patch());
            writes += 1;
        }
        let result = c.sector_io(r.first, r.count, r.fua, r.op());
        assert!(result.is_ok());
        let reply = r.response(&result)?;
        assert_eq!(reply[16], 0);
        if op == READ {
            assert!(reply[HEADER..] == read_image[OFFSET..OFFSET + LENGTH]);
            let counts = (
                c.counts.origin_reads - prior.origin_reads,
                c.counts.cache_reads - prior.cache_reads,
            );
            if i == 0 {
                assert_eq!(counts, (2, 0));
                assert_eq!(c.origin.stream_position()?, 8192);
            } else {
                assert_eq!(counts, (0, 2));
                assert_eq!(c.origin.stream_position()?, 1);
                assert_eq!(c.cache.stream_position()?, 8192);
            }
        } else {
            assert_eq!(reply.len(), HEADER);
            check_file(&mut c.origin, &image(true))?;
            check_file(&mut c.cache, &original()[..8192])?;
        }
        send(&mut output, &reply)?;
    }
    // The client request and this DLL shutdown barrier have distinct completions.
    let mut tag = [0; 4];
    input.read_exact(&mut tag)?;
    assert_eq!(u32::from_le_bytes(tag), 0x464c5348);
    let outcome = c.sector_io(0, 0, false, SectorOp::Flush);
    let error = outcome
        .as_ref()
        .err()
        .and_then(io::Error::raw_os_error)
        .unwrap_or(if outcome.is_ok() { 0 } else { 1 });
    output.write_all(&(error as u32).to_le_bytes())?;
    output.flush()?;
    assert_eq!(outcome?, SectorReply::Flushed);
    let status = broker.0.wait()?;
    drop(output);
    let mut logs = String::new();
    broker.0.stderr.take().unwrap().read_to_string(&mut logs)?;
    assert!(status.success(), "broker {status}: {logs}");
    let mut surplus = Vec::new();
    input.read_to_end(&mut surplus)?;
    assert!(surplus.is_empty());
    assert!(
        logs.contains("BROKER peer-verified")
            && logs.contains("SDK ready inner-peer=verified dispatchers=1")
    );
    let external = logs
        .lines()
        .filter(|l| l.starts_with("SDK outer-request "))
        .count();
    assert_eq!(external, if recover { 2 } else { 3 });
    assert_eq!(
        logs.lines()
            .filter(|l| l.starts_with("SDK callback "))
            .count(),
        sequence.len()
    );
    assert!(
        logs.contains(&format!(
            "SDK stopped callbacks={} rejected=0 close_calls=1 close_error=0 waited=1",
            sequence.len()
        )),
        "{logs}"
    );
    if recover {
        assert!(logs.contains("BROKER client-result=0 reaped=1"));
    } else {
        assert!(
            logs.contains(&format!(
                "CUT write after_apply={} write_response_sent=0",
                u8::from(applied)
            )),
            "{logs}"
        );
        assert!(
            logs.contains("CLIENT_WRITE_TRANSPORT_ERROR lba=7 sectors=2 reaped=1 "),
            "{logs}"
        );
    }
    assert_eq!(
        (
            c.counts.origin_reads,
            c.counts.cache_reads,
            c.counts.origin_syncs
        ),
        (2, 2, 1)
    );
    check_file(&mut c.origin, &prefix_image(prefix_len))?;
    check_file(&mut c.cache, &read_image[..8192])?;
    for line in logs.lines().filter(|l| {
        l.starts_with("CLIENT_WRITE_TRANSPORT_ERROR")
            || l.starts_with("CUT write")
            || l.starts_with("BROKER client-result=")
    }) {
        println!("RECOVERY_EVENT {name} {line}");
    }
    println!(
        "RECOVERY_WORKER {name} received={external} callbacks={} applied_writes={writes} cold_origin_lines=2 warm_cache_lines=2 shutdown_sync=1",
        sequence.len()
    );
    Ok(())
}

#[test]
#[ignore = "run only as an owned worker of the recovery experiment"]
fn recovery_worker() -> io::Result<()> {
    if !exact(WORKER)? {
        return Ok(());
    }
    let mut msg = String::new();
    io::stdin().take(96).read_to_string(&mut msg)?;
    let fields: Vec<_> = msg.split_whitespace().collect();
    assert_eq!(fields.len(), 4);
    assert_eq!(fields[0], "RECOVERY_V1");
    exercise_worker(
        fields[1].parse().expect("fixture owner"),
        fields[2].parse().expect("fixture serial"),
        fields[3],
    )
}

fn run_worker(images: &Images, role: &str) -> io::Result<()> {
    let mut child = Command::new(std::env::current_exe()?)
        .args(["--ignored", "--exact", WORKER, "--nocapture"])
        .current_dir(&images.dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let owner = std::process::id();
    let prefix = format!("win-cache-file-test-{owner}-");
    let serial = images
        .dir
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .strip_prefix(&prefix)
        .unwrap();
    let control = writeln!(
        child.stdin.take().unwrap(),
        "RECOVERY_V1 {owner} {serial} {role}"
    );
    let out = child.wait_with_output()?;
    control?;
    let receipt = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "worker {role} {}: {} {}",
        out.status,
        receipt,
        String::from_utf8_lossy(&out.stderr)
    );
    let marker = format!("RECOVERY_WORKER {role} ");
    assert_eq!(
        receipt.lines().filter(|l| l.starts_with(&marker)).count(),
        1,
        "{receipt}"
    );
    for line in receipt.lines().filter(|l| l.starts_with("RECOVERY_")) {
        println!("{line}");
    }
    Ok(())
}

fn pin_files(images: &Images) -> io::Result<(File, File, File)> {
    // Deny deletion or renaming of the directory and both files across worker lifetimes.
    let dir = OpenOptions::new()
        .read(true)
        .share_mode(3)
        .custom_flags(0x0200_0000)
        .open(&images.dir)?;
    let guard = |n| {
        OpenOptions::new()
            .read(true)
            .share_mode(3)
            .open(images.dir.join(n))
    };
    Ok((dir, guard("origin.img")?, guard("cache.img")?))
}

#[test]
#[ignore = "requires the pinned official Windows client and server DLL"]
fn restart_after_lost_write() -> io::Result<()> {
    if !exact(PARENT)? {
        return Ok(());
    }
    for (loss, reopen, applied) in [
        ("after-loss", "after-reopen", true),
        ("before-loss", "before-reopen", false),
    ] {
        let old = original();
        let expected = image(applied);
        let images = Images::new(&old)?;
        let (_dir, mut origin, mut cache) = pin_files(&images)?;
        run_worker(&images, loss)?;
        check_file(&mut origin, &expected)?;
        check_file(&mut cache, &old[..8192])?;
        run_worker(&images, reopen)?;
        check_file(&mut origin, &expected)?;
        check_file(&mut cache, &expected[..8192])?;
        let version = if applied { "new" } else { "old" };
        println!(
            "RECOVERY_PAIR applied={} client_write_response=absent before_reopen_origin={version} before_reopen_cache=old first_read={version} cold_origin_lines=2 warm_cache_lines=2 rust_workers_reaped=2 final_image=verified",
            u8::from(applied)
        );
    }
    println!("FILE_CACHE_RECOVERY_OK");
    Ok(())
}

#[path = "abrupt_checks.rs"]
mod abrupt;
