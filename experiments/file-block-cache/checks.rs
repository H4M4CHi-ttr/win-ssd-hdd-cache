use super::*;
use std::fs::{self, DirBuilder, OpenOptions};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

const END: usize = 2 * BLOCK_BYTES + 23;
const CHILD_ARGS: [&str; 4] = [
    "--ignored",
    "--exact",
    "cache::tests::restart_child_with_inherited_images",
    "--nocapture",
];
const RESTART_TEST: &str = "cache::tests::cold_restart_with_stale_cache_file";
static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

fn data(seed: u8) -> Vec<u8> {
    (0..END)
        .map(|i| seed.wrapping_add(((i % 251 + i / BLOCK_BYTES * 61) % 256) as u8))
        .collect()
}

struct Images {
    dir: PathBuf,
    owns_origin: bool,
    owns_cache: bool,
}

impl Images {
    fn new(initial: &[u8]) -> io::Result<Self> {
        let builder = DirBuilder::new();
        #[cfg(unix)]
        let builder = {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = builder;
            builder.mode(0o700);
            builder
        };
        let dir = loop {
            let id = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
            let name = format!("win-cache-file-test-{}-{id}", std::process::id());
            let path = std::env::temp_dir().join(name);
            match builder.create(&path) {
                Ok(()) => break path,
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        };
        let mut images = Self {
            dir,
            owns_origin: false,
            owns_cache: false,
        };
        let mut origin = File::create_new(images.dir.join("origin.img"))?;
        images.owns_origin = true;
        origin.write_all(initial)?;
        let cache = File::create_new(images.dir.join("cache.img"))?;
        images.owns_cache = true;
        cache.set_len((2 * BLOCK_BYTES) as u64)?;
        Ok(images)
    }

    fn open(&self, name: &str, read: bool, write: bool) -> io::Result<File> {
        OpenOptions::new()
            .read(read)
            .write(write)
            .open(self.dir.join(name))
    }

    fn session(&self) -> io::Result<FileCache> {
        FileCache::new(
            self.open("origin.img", true, true)?,
            self.open("cache.img", true, true)?,
        )
    }

    fn cache_bytes(&self, offset: u64, len: usize) -> io::Result<Vec<u8>> {
        let mut f = self.open("cache.img", true, false)?;
        f.seek(SeekFrom::Start(offset))?;
        let mut out = vec![0; len];
        f.read_exact(&mut out)?;
        Ok(out)
    }
}

impl Drop for Images {
    fn drop(&mut self) {
        let cleanup = || -> io::Result<()> {
            if self.owns_cache {
                fs::remove_file(self.dir.join("cache.img"))?;
            }
            if self.owns_origin {
                fs::remove_file(self.dir.join("origin.img"))?;
            }
            fs::remove_dir(&self.dir)
        };
        if let Err(e) = cleanup() {
            if std::thread::panicking() {
                eprintln!("fixture cleanup also failed: {e}");
            } else {
                panic!("fixture cleanup failed: {e}");
            }
        }
    }
}

fn expect(r: BlockRead, from: ReadFrom, expected: &[u8]) {
    assert_eq!(r.from, from);
    assert_eq!(r.bytes.len(), expected.len());
    assert!(
        r.bytes == expected,
        "file data differs from the independent expected bytes"
    );
}

fn same_error(got: io::Error, direct: io::Error) {
    assert_eq!(
        (got.kind(), got.raw_os_error()),
        (direct.kind(), direct.raw_os_error())
    );
}

#[test]
fn warm_hit_reads_the_file_with_origin_unreadable() -> io::Result<()> {
    let initial = data(17);
    let images = Images::new(&initial)?;
    let mut c = images.session()?;
    let expected = &initial[BLOCK_BYTES..2 * BLOCK_BYTES];
    expect(c.read_block(1)?, ReadFrom::Origin, expected);
    assert_eq!(
        images.cache_bytes(BLOCK_BYTES as u64, BLOCK_BYTES)?,
        expected
    );
    // Turn any unexpected origin read into a real OS error without changing the data.
    c.origin = images.open("origin.img", false, true)?;
    assert!(
        images
            .open("origin.img", false, true)?
            .read(&mut [0])
            .is_err()
    );
    // A duplicated handle shares the OS file position. A real read of slot 1 must move it.
    let mut cache_cursor = c.cache.try_clone()?;
    cache_cursor.seek(SeekFrom::Start(1))?;
    let before = c.counts;
    expect(c.read_block(1)?, ReadFrom::Cache, expected);
    assert_eq!(cache_cursor.stream_position()?, (2 * BLOCK_BYTES) as u64);
    assert_eq!(c.counts.origin_reads, before.origin_reads);
    assert_eq!(c.counts.cache_reads, before.cache_reads + 1);
    Ok(())
}

#[test]
fn an_overlapping_write_rejects_a_late_physical_fill() -> io::Result<()> {
    let old = data(11);
    let mut expected = old.clone();
    expected[19..23].copy_from_slice(b"new!");
    let images = Images::new(&old)?;
    let mut c = images.session()?;
    c.begin_fill(0)?;
    c.write_at(19, b"new!")?;
    assert_eq!(c.complete_fill()?, Publication::Stale);
    assert_eq!(images.cache_bytes(0, BLOCK_BYTES)?, old[..BLOCK_BYTES]);
    expect(c.read_block(0)?, ReadFrom::Origin, &expected[..BLOCK_BYTES]);
    expect(c.read_block(0)?, ReadFrom::Cache, &expected[..BLOCK_BYTES]);
    Ok(())
}

#[test]
fn a_nonoverlapping_write_preserves_the_held_fill() -> io::Result<()> {
    let expected = data(25);
    let images = Images::new(&expected)?;
    let mut c = images.session()?;
    c.begin_fill(0)?;
    c.write_at(31, &[])?;
    c.write_at(BLOCK_BYTES as u64, b"next")?;
    assert_eq!(c.complete_fill()?, Publication::Published);
    c.origin = images.open("origin.img", false, true)?;
    expect(c.read_block(0)?, ReadFrom::Cache, &expected[..BLOCK_BYTES]);
    Ok(())
}

#[test]
fn a_colliding_read_cannot_reuse_the_reserved_slot() -> io::Result<()> {
    let old = data(31);
    let images = Images::new(&old)?;
    let mut c = images.session()?;
    c.begin_fill(0)?;
    c.write_at(0, b"x")?;
    assert_eq!(
        c.begin_fill(2).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    expect(c.read_block(2)?, ReadFrom::Origin, &old[2 * BLOCK_BYTES..]);
    assert_eq!(images.cache_bytes(0, 23)?, vec![0; 23]);
    assert_eq!(c.complete_fill()?, Publication::Stale);
    assert_eq!(images.cache_bytes(0, BLOCK_BYTES)?, old[..BLOCK_BYTES]);
    expect(c.read_block(2)?, ReadFrom::Origin, &old[2 * BLOCK_BYTES..]);
    expect(c.read_block(2)?, ReadFrom::Cache, &old[2 * BLOCK_BYTES..]);
    let mut current = old[..BLOCK_BYTES].to_vec();
    current[0] = b'x';
    expect(c.read_block(0)?, ReadFrom::Origin, &current);
    Ok(())
}

#[test]
fn cross_block_write_invalidates_both_lines() -> io::Result<()> {
    let mut expected = data(57);
    let images = Images::new(&expected)?;
    let mut c = images.session()?;
    for block in 0..2 {
        let range = block * BLOCK_BYTES..(block + 1) * BLOCK_BYTES;
        expect(
            c.read_block(block as u64)?,
            ReadFrom::Origin,
            &expected[range.clone()],
        );
        expect(
            c.read_block(block as u64)?,
            ReadFrom::Cache,
            &expected[range],
        );
    }
    c.write_at((BLOCK_BYTES - 1) as u64, b"XY")?;
    expected[BLOCK_BYTES - 1..BLOCK_BYTES + 1].copy_from_slice(b"XY");
    assert_eq!(fs::read(images.dir.join("origin.img"))?, expected);
    for block in 0..2 {
        expect(
            c.read_block(block as u64)?,
            ReadFrom::Origin,
            &expected[block * BLOCK_BYTES..(block + 1) * BLOCK_BYTES],
        );
    }
    Ok(())
}

#[test]
fn short_cache_read_discards_partial_bytes_and_falls_back() -> io::Result<()> {
    let expected = data(75);
    let images = Images::new(&expected)?;
    let mut c = images.session()?;
    expect(c.read_block(0)?, ReadFrom::Origin, &expected[..BLOCK_BYTES]);
    let mut fault = images.open("cache.img", false, true)?;
    fault.set_len(3)?;
    fault.write_all(b"bad")?;
    drop(fault);
    expect(c.read_block(0)?, ReadFrom::Origin, &expected[..BLOCK_BYTES]);
    assert_eq!(
        (
            c.counts.cache_reads,
            c.counts.origin_reads,
            c.counts.cache_read_errors
        ),
        (1, 2, 1)
    );
    expect(c.read_block(0)?, ReadFrom::Cache, &expected[..BLOCK_BYTES]);
    Ok(())
}

#[test]
fn fallback_returns_the_real_origin_error() -> io::Result<()> {
    let expected = data(82);
    let images = Images::new(&expected)?;
    let mut c = images.session()?;
    expect(c.read_block(0)?, ReadFrom::Origin, &expected[..BLOCK_BYTES]);
    images.open("cache.img", false, true)?.set_len(3)?;
    c.origin = images.open("origin.img", false, true)?;
    let direct = images
        .open("origin.img", false, true)?
        .read_exact(&mut [0])
        .unwrap_err();
    assert_ne!(direct.kind(), io::ErrorKind::UnexpectedEof);
    same_error(c.read_block(0).unwrap_err(), direct);
    assert_eq!(
        (
            c.counts.cache_reads,
            c.counts.origin_reads,
            c.counts.cache_read_errors
        ),
        (1, 2, 1)
    );
    Ok(())
}

#[test]
fn cache_fill_write_failure_keeps_the_origin_result_and_a_miss() -> io::Result<()> {
    let expected = data(93);
    let images = Images::new(&expected)?;
    let mut c = FileCache::new(
        images.open("origin.img", true, true)?,
        images.open("cache.img", true, false)?,
    )?;
    for _ in 0..2 {
        expect(c.read_block(0)?, ReadFrom::Origin, &expected[..BLOCK_BYTES]);
    }
    assert_eq!(
        (
            c.counts.origin_reads,
            c.counts.cache_reads,
            c.counts.cache_write_errors
        ),
        (2, 0, 2)
    );
    assert_eq!(images.cache_bytes(0, BLOCK_BYTES)?, vec![0; BLOCK_BYTES]);
    Ok(())
}

#[test]
fn failed_origin_write_keeps_both_overlapping_lines_invalid() -> io::Result<()> {
    let expected = data(104);
    let images = Images::new(&expected)?;
    let mut c = FileCache::new(
        images.open("origin.img", true, false)?,
        images.open("cache.img", true, true)?,
    )?;
    for block in 0..2 {
        let line = &expected[block * BLOCK_BYTES..(block + 1) * BLOCK_BYTES];
        expect(c.read_block(block as u64)?, ReadFrom::Origin, line);
        expect(c.read_block(block as u64)?, ReadFrom::Cache, line);
    }
    let mut probe = images.open("origin.img", true, false)?;
    probe.seek(SeekFrom::Start((BLOCK_BYTES - 1) as u64))?;
    let direct = probe.write_all(b"XY").unwrap_err();
    same_error(
        c.write_at((BLOCK_BYTES - 1) as u64, b"XY").unwrap_err(),
        direct,
    );
    for block in 0..2 {
        expect(
            c.read_block(block as u64)?,
            ReadFrom::Origin,
            &expected[block * BLOCK_BYTES..(block + 1) * BLOCK_BYTES],
        );
    }
    assert_eq!(fs::read(images.dir.join("origin.img"))?, expected);
    Ok(())
}

#[test]
fn short_tail_and_rejected_ranges_do_not_extend_the_origin() -> io::Result<()> {
    let expected = data(120);
    let images = Images::new(&expected)?;
    let mut c = images.session()?;
    expect(c.read_block(0)?, ReadFrom::Origin, &expected[..BLOCK_BYTES]);
    let tail = &expected[2 * BLOCK_BYTES..];
    expect(c.read_block(2)?, ReadFrom::Origin, tail);
    expect(c.read_block(2)?, ReadFrom::Cache, tail);
    c.write_at(END as u64, &[])?;
    for (offset, bytes) in [
        (END as u64 - 1, b"XX".as_slice()),
        (u64::MAX, b"X"),
        (END as u64 + 1, b""),
    ] {
        assert_eq!(
            c.write_at(offset, bytes).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
    }
    for block in [3, u64::MAX] {
        assert_eq!(
            c.read_block(block).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
    }
    expect(c.read_block(2)?, ReadFrom::Cache, tail);
    assert_eq!(fs::read(images.dir.join("origin.img"))?, expected);
    Ok(())
}

// Only the parent creates paths. A child receives the same ordinary files as OS handles:
// stdin carries the writable cache and stderr the read-only origin; stdout carries the receipt.
#[cfg(unix)]
fn inherited_files() -> io::Result<(File, File)> {
    use std::os::fd::AsFd;
    Ok((
        File::from(io::stderr().as_fd().try_clone_to_owned()?),
        File::from(io::stdin().as_fd().try_clone_to_owned()?),
    ))
}

#[cfg(windows)]
fn inherited_files() -> io::Result<(File, File)> {
    use std::os::windows::io::AsHandle;
    Ok((
        File::from(io::stderr().as_handle().try_clone_to_owned()?),
        File::from(io::stdin().as_handle().try_clone_to_owned()?),
    ))
}

fn run_child(images: &Images, role: &str) -> io::Result<()> {
    let output = Command::new(std::env::current_exe()?)
        .args(CHILD_ARGS)
        .stdin(Stdio::from(images.open("cache.img", true, true)?))
        .stderr(Stdio::from(images.open("origin.img", true, false)?))
        .stdout(Stdio::piped())
        .output()?;
    let log = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success()
            && log
                .lines()
                .any(|l| l.ends_with(&format!("FILE_CACHE_CHILD_OK {role}"))),
        "child {role} did not verify its stage (status {}): {log}",
        output.status
    );
    Ok(())
}

#[test]
#[ignore = "the restart parent supplies both fixture file handles"]
fn restart_child_with_inherited_images() -> io::Result<()> {
    // Even --include-ignored in a normal run must never adopt inherited files.
    if !std::env::args().skip(1).eq(CHILD_ARGS) {
        return Ok(());
    }
    let (origin, cache) = inherited_files()?;
    let mut c = FileCache::new(origin, cache)?;
    assert_eq!(c.capacity, END as u64);
    assert_eq!(c.cache.metadata()?.len(), (2 * BLOCK_BYTES) as u64);

    // Check the shape and contents of both synthetic inputs before any cache fill.
    // The prior cache bytes themselves distinguish the parent's two independent stages.
    let before = data(12);
    let mut previous = vec![0; 2 * BLOCK_BYTES];
    c.cache.rewind()?;
    c.cache.read_exact(&mut previous)?;
    let (role, expected) = if previous.iter().all(|&b| b == 0) {
        ("warm", before)
    } else {
        assert!(
            previous[..BLOCK_BYTES] == before[..BLOCK_BYTES]
                && previous[BLOCK_BYTES..].iter().all(|&b| b == 0),
            "unexpected input cache"
        );
        ("restart", data(241))
    };
    let mut original = vec![0; END];
    c.origin.rewind()?;
    c.origin.read_exact(&mut original)?;
    assert!(original == expected, "unexpected input origin");

    expect(c.read_block(0)?, ReadFrom::Origin, &expected[..BLOCK_BYTES]);
    expect(c.read_block(0)?, ReadFrom::Cache, &expected[..BLOCK_BYTES]);
    assert_eq!((c.counts.origin_reads, c.counts.cache_reads), (1, 1));
    println!("FILE_CACHE_CHILD_OK {role}");
    Ok(())
}

#[test]
fn cold_restart_with_stale_cache_file() -> io::Result<()> {
    let before = data(12);
    let after = data(241);
    let images = Images::new(&before)?;
    run_child(&images, "warm")?;
    assert_eq!(images.cache_bytes(0, BLOCK_BYTES)?, before[..BLOCK_BYTES]);
    images.open("origin.img", false, true)?.write_all(&after)?;
    assert_eq!(images.cache_bytes(0, BLOCK_BYTES)?, before[..BLOCK_BYTES]);
    run_child(&images, "restart")?;
    assert_eq!(images.cache_bytes(0, BLOCK_BYTES)?, after[..BLOCK_BYTES]);
    println!("two child processes verified: warm A, stale A on disk, cold B, warm B");
    Ok(())
}

#[test]
fn the_restart_parent_ignores_ambient_roles_and_redirected_files() -> io::Result<()> {
    let before = data(12);
    let decoy = Images::new(&before)?;
    let output = Command::new(std::env::current_exe()?)
        .args(["--exact", RESTART_TEST, "--nocapture"])
        .env("WIN_CACHE_FILE_EXPERIMENT_CHILD", "warm")
        .stdin(Stdio::from(decoy.open("cache.img", true, true)?))
        .stderr(Stdio::from(decoy.open("origin.img", true, false)?))
        .output()?;
    assert!(output.status.success());
    let bytes = decoy.cache_bytes(0, 2 * BLOCK_BYTES)?;
    assert!(
        bytes.iter().all(|&b| b == 0),
        "test used the redirected file as its cache"
    );
    assert_eq!(fs::read(decoy.dir.join("origin.img"))?, before);
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains("two child processes verified: warm A, stale A on disk, cold B, warm B")
    );
    Ok(())
}

#[path = "sector_checks.rs"]
mod sector_checks;

#[path = "winspd/pipe_checks.rs"]
mod winspd;

#[path = "workload_checks.rs"]
mod workload_checks;
