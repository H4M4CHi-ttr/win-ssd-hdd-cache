//! Write protection on the provider's user-mode pipe, with an immutable origin epoch.
use super::live::{receive, send, start_broker};
use super::*;
use std::os::windows::fs::OpenOptionsExt;

const NAME: &str = "cache::tests::winspd::read_only::official_write_protection";

fn run_case(name: &str, reads: usize, denied: Option<(u8, u8, u8)>, fua: bool) -> io::Result<()> {
    let initial: Vec<u8> = (0..131 * 512)
        .map(|i| ((i % 251 + i / 512 * 47) % 256) as u8)
        .collect();
    let images = Images::new(&initial)?;
    // Only this owner may open for writes. Its sole I/O operation is sync_all, which
    // requires write access on Windows. The actual read path gets a read-only handle.
    let open_delete = || {
        OpenOptions::new()
            .read(true)
            .access_mode(0x0001_0000)
            .share_mode(7)
            .open(images.dir.join("origin.img"))
    };
    drop(open_delete()?); // Access-only check; never request delete-on-close.
    let barrier = OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(1)
        .open(images.dir.join("origin.img"))?;
    let rejected_open = images.open("origin.img", false, true).unwrap_err();
    assert_eq!(rejected_open.raw_os_error(), Some(32));
    assert_eq!(open_delete().unwrap_err().raw_os_error(), Some(32));
    let mut c = FileCache::new(
        images.open("origin.img", true, false)?,
        images.open("cache.img", true, true)?,
    )?;
    let mut broker = start_broker(name)?;
    let mut input = broker.0.stdout.take().unwrap();
    let mut output = broker.0.stdin.take().unwrap();
    for index in 0..reads {
        let bytes = receive(&mut input)?;
        let request = Request::decode(&bytes)?;
        assert_eq!(
            (
                request.kind,
                request.hint as u32,
                request.first,
                request.count,
                request.fua
            ),
            (READ, index as u32, 7, 2, fua)
        );
        let prev = c.counts;
        c.origin.seek(SeekFrom::Start(1))?;
        let result = c.sector_io_with_sync(7, 2, fua, request.op(), |_| barrier.sync_all());
        let reply = request.response(&result)?;
        assert!(result.is_ok() && reply[16] == 0 && reply[HEADER..] == initial[3584..4608]);
        let origin_end = c.origin.stream_position()?;
        let delta = (
            c.counts.origin_reads - prev.origin_reads,
            c.counts.cache_reads - prev.cache_reads,
        );
        if fua {
            assert_eq!((origin_end, delta), (4608, (1, 0)));
        } else if index == 0 {
            assert_eq!((origin_end, delta), (8192, (2, 0)));
        } else {
            assert_eq!((origin_end, delta), (1, (0, 2)));
        }
        send(&mut output, &reply)?;
    }

    let external_syncs = c.counts.origin_syncs;
    assert_eq!(external_syncs, if fua { reads as u64 } else { 0 });
    // With CacheSupported set, the next Rust message must be the separate shutdown
    // barrier. A forwarded mutation would appear here as a normal length-prefixed request.
    if !fua {
        let mut tag = [0; 4];
        input.read_exact(&mut tag)?;
        assert_eq!(
            u32::from_le_bytes(tag),
            0x464c5348,
            "unexpected operation reached the file backend"
        );
        let result = c.sector_io_with_sync(0, 0, false, SectorOp::Flush, |_| barrier.sync_all());
        let error = result
            .as_ref()
            .err()
            .and_then(io::Error::raw_os_error)
            .unwrap_or(if result.is_ok() { 0 } else { 1 });
        output.write_all(&(error as u32).to_le_bytes())?;
        output.flush()?;
        assert_eq!(result?, SectorReply::Flushed);
    }
    let status = broker.0.wait()?;
    drop(output);
    let mut logs = String::new();
    broker.0.stderr.take().unwrap().read_to_string(&mut logs)?;
    let mut extra = Vec::new();
    input.read_to_end(&mut extra)?;
    assert!(status.success() && extra.is_empty(), "{status} {logs}");
    assert!(
        logs.contains("BROKER peer-verified")
            && logs.contains("SDK ready inner-peer=verified dispatchers=1")
    );
    assert!(logs.contains("SDK advertised write_protected=1 unmap=0"));
    let total = reads + usize::from(denied.is_some());
    assert_eq!(
        logs.lines()
            .filter(|l| l.starts_with("SDK outer-request "))
            .count(),
        total
    );
    assert_eq!(
        logs.lines()
            .filter(|l| l.starts_with("SDK callback "))
            .count(),
        reads
    );
    assert_eq!(
        logs.lines()
            .filter(|l| l.starts_with("PROTECTED_REPLY "))
            .count(),
        usize::from(denied.is_some())
    );
    assert_eq!(
        logs.lines()
            .filter(|l| l.starts_with("SDK protection-denied "))
            .count(),
        usize::from(denied.is_some())
    );
    assert!(logs.contains(&format!(
        "SDK stopped callbacks={reads} rejected=0 close_calls={} close_error=0 waited=1",
        u8::from(!fua)
    )));
    let mut denial = String::from("none");
    if let Some((kind, key, asc)) = denied {
        assert!(
            logs.contains(&format!(
                "PROTECTED_REPLY kind={kind} scsi=2 sense={key:02x}/{asc:02x} bytes=0"
            )),
            "{logs}"
        );
        assert!(logs.contains("BROKER client-result=1117 reaped=1"));
        if kind == WRITE {
            // The pinned client's first byte is distinct from the unchanged origin at LBA 7.
            assert_ne!(initial[3584], 0x87);
            assert!(
                logs.contains(&format!(
                    "PROTECTED_WRITE lba=7 count=2 first_byte=135 fua={}",
                    u8::from(fua)
                )),
                "{logs}"
            );
        }
        denial = format!("{key:02x}/{asc:02x}");
    } else {
        assert!(logs.contains("BROKER client-result=0 reaped=1"));
    }
    assert_eq!(
        c.counts.origin_syncs,
        u64::from(!fua) + if fua { reads as u64 } else { 0 }
    );
    assert!(fs::read(images.dir.join("origin.img"))? == initial);
    let cache_bytes = images.cache_bytes(0, 8192)?;
    if !fua && reads > 0 {
        assert!(cache_bytes == initial[..8192]);
    } else {
        assert!(cache_bytes.iter().all(|&v| v == 0));
    }
    println!(
        "READ_ONLY {name} advertised=1 external={total} backend_reads={reads} backend_mutations=0 sense={denial} origin_reads={} cache_reads={} external_syncs={external_syncs} shutdown_syncs={} final_origin_verified=1 conflicting_write_and_delete_denied=1",
        c.counts.origin_reads,
        c.counts.cache_reads,
        c.counts.origin_syncs - external_syncs
    );
    Ok(())
}

#[test]
#[ignore = "the staged runner supplies the pinned client and server DLL"]
fn official_write_protection() -> io::Result<()> {
    if !std::env::args()
        .skip(1)
        .eq(["--ignored", "--exact", NAME, "--nocapture"])
    {
        return Ok(());
    }
    assert_eq!(
        std::env::current_exe()?.file_name().unwrap(),
        "file-cache-pipe-test.exe"
    );
    for (name, reads, denied, fua) in [
        ("ro-write", 2, Some((WRITE, 7, 0x27)), false),
        ("ro-warm", 2, None, false),
        ("ro-unmap", 2, Some((UNMAP, 5, 0x20)), false),
        ("ro-fua", 1, None, true),
        ("ro-flush", 0, Some((FLUSH, 7, 0x27)), false),
        ("ro-nocache-flush", 0, Some((FLUSH, 5, 0x20)), true),
        ("ro-fua-write", 0, Some((WRITE, 7, 0x27)), true),
    ] {
        run_case(name, reads, denied, fua)?;
    }
    println!("FILE_CACHE_READ_ONLY_OK");
    Ok(())
}
