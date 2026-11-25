use super::*;

fn sector_data(sectors: usize, version: u8) -> Vec<u8> {
    (0..sectors * 512)
        .map(|i| version.wrapping_add(((i / 512 * 37 + i % 241) % 256) as u8))
        .collect()
}

fn expect_data(reply: SectorReply, bytes: &[u8]) {
    let SectorReply::Data(actual) = reply else {
        panic!("expected a read reply")
    };
    assert_eq!(actual.len(), bytes.len());
    assert!(
        actual == bytes,
        "sector data differs from the independent bytes"
    );
}

#[test]
fn sector_rewrites_cross_a_cache_line_and_reach_both_files() -> io::Result<()> {
    // LBAs 7 and 8 straddle the 4096-byte cache line boundary.
    let mut origin = sector_data(19, 31);
    let images = Images::new(&origin)?;
    let mut c = images.session()?;
    for version in [83, 179] {
        let patch = sector_data(2, version);
        assert_eq!(
            c.sector_io(7, 2, false, SectorOp::Write(&patch))?,
            SectorReply::Written
        );
        origin[3584..4608].copy_from_slice(&patch);
        assert!(fs::read(images.dir.join("origin.img"))? == origin);

        let before = c.counts;
        expect_data(
            c.sector_io(6, 4, false, SectorOp::Read)?,
            &origin[3072..5120],
        );
        assert_eq!(c.counts.origin_reads - before.origin_reads, 2);
        assert!(images.cache_bytes(0, 8192)? == origin[..8192]);
        c.origin = images.open("origin.img", false, true)?;
        expect_data(
            c.sector_io(7, 2, false, SectorOp::Read)?,
            &origin[3584..4608],
        );
        assert_eq!(c.counts.cache_reads - before.cache_reads, 2);
        c.origin = images.open("origin.img", true, true)?;
    }
    Ok(())
}

#[test]
fn sector_limit_and_last_complete_sector_use_the_exact_length() -> io::Result<()> {
    let mut origin = sector_data(131, 7);
    let images = Images::new(&origin)?;
    let mut c = images.session()?;
    let patch = sector_data(128, 131);
    assert_eq!(
        c.sector_io(3, 128, false, SectorOp::Write(&patch))?,
        SectorReply::Written
    );
    origin[1536..].copy_from_slice(&patch);
    expect_data(c.sector_io(3, 128, false, SectorOp::Read)?, &origin[1536..]);
    expect_data(
        c.sector_io(130, 1, false, SectorOp::Read)?,
        &origin[66560..67072],
    );
    assert!(fs::read(images.dir.join("origin.img"))? == origin);
    Ok(())
}

#[test]
fn sector_rejections_leave_the_origin_and_the_warm_cache_untouched() -> io::Result<()> {
    let original = sector_data(160, 3);
    let images = Images::new(&original)?;
    let mut c = images.session()?;
    expect_data(c.sector_io(0, 1, false, SectorOp::Read)?, &original[..512]);
    let resident = images.cache_bytes(0, 8192)?;
    let before = c.counts;
    let bad = io::ErrorKind::InvalidInput;
    let unhandled = io::ErrorKind::Unsupported;
    for (lba, count, fua, op, error) in [
        (0, 1, false, SectorOp::Write(&[9; 511]), bad),
        (0, 1, false, SectorOp::Write(&[9; 513]), bad),
        (159, 2, false, SectorOp::Write(&[9; 1024]), bad),
        (159, 2, false, SectorOp::Read, bad),
        (u64::MAX, 1, false, SectorOp::Read, bad),
        (0, u32::MAX, false, SectorOp::Read, bad),
        (0, 129, false, SectorOp::Read, bad),
        (0, 0, false, SectorOp::Read, bad),
        (0, 0, false, SectorOp::Write(&[]), bad),
        (160, 1, true, SectorOp::Read, bad),
        (0, 1, true, SectorOp::Write(&[9; 511]), bad),
        (159, 2, false, SectorOp::Flush, bad),
        (0, 1, false, SectorOp::Unmap, unhandled),
    ] {
        assert_eq!(c.sector_io(lba, count, fua, op).unwrap_err().kind(), error);
    }
    assert_eq!(
        (c.counts.origin_reads, c.counts.cache_reads),
        (before.origin_reads, before.cache_reads)
    );
    assert!(fs::read(images.dir.join("origin.img"))? == original);
    assert!(images.cache_bytes(0, 8192)? == resident);
    c.origin = images.open("origin.img", false, true)?;
    expect_data(c.sector_io(0, 1, false, SectorOp::Read)?, &original[..512]);
    Ok(())
}

#[test]
fn sector_geometry_rejects_an_empty_or_fractional_image() -> io::Result<()> {
    for original in [vec![], vec![0x35; 513]] {
        let images = Images::new(&original)?;
        let mut c = images.session()?;
        for op in [SectorOp::Read, SectorOp::Write(&[9; 512])] {
            assert_eq!(
                c.sector_io(0, 1, false, op).unwrap_err().kind(),
                io::ErrorKind::InvalidInput
            );
        }
        assert_eq!((c.counts.origin_reads, c.counts.cache_reads), (0, 0));
        assert!(fs::read(images.dir.join("origin.img"))? == original);
        assert!(images.cache_bytes(0, 8192)?.iter().all(|&v| v == 0));
    }
    Ok(())
}

#[test]
fn sector_read_returns_the_second_lines_origin_error_without_partial_success() -> io::Result<()> {
    let original = sector_data(16, 29);
    let images = Images::new(&original)?;
    let mut c = images.session()?;
    expect_data(
        c.sector_io(7, 1, false, SectorOp::Read)?,
        &original[3584..4096],
    );
    c.origin = images.open("origin.img", false, true)?;
    let direct = images
        .open("origin.img", false, true)?
        .read_exact(&mut [0])
        .unwrap_err();
    let before = c.counts;
    same_error(
        c.sector_io(7, 2, false, SectorOp::Read).unwrap_err(),
        direct,
    );
    assert_eq!(
        (
            c.counts.cache_reads - before.cache_reads,
            c.counts.origin_reads - before.origin_reads
        ),
        (1, 1)
    );
    c.origin = images.open("origin.img", true, true)?;
    expect_data(
        c.sector_io(7, 2, false, SectorOp::Read)?,
        &original[3584..4608],
    );
    Ok(())
}

#[test]
fn a_sector_write_invalidates_an_already_sampled_fill() -> io::Result<()> {
    let old = sector_data(16, 47);
    let fresh = sector_data(1, 203);
    let images = Images::new(&old)?;
    let mut c = images.session()?;
    c.begin_fill(1)?;
    assert_eq!(
        c.sector_io(8, 1, false, SectorOp::Write(&fresh))?,
        SectorReply::Written
    );
    assert_eq!(c.complete_fill()?, Publication::Stale);
    assert!(images.cache_bytes(4096, 4096)? == old[4096..8192]);
    let before = c.counts;
    expect_data(c.sector_io(8, 1, false, SectorOp::Read)?, &fresh);
    assert_eq!(c.counts.origin_reads - before.origin_reads, 1);
    c.origin = images.open("origin.img", false, true)?;
    expect_data(c.sector_io(8, 1, false, SectorOp::Read)?, &fresh);
    Ok(())
}

#[test]
fn fua_read_synchronizes_then_uses_only_the_origin() -> io::Result<()> {
    let original = sector_data(16, 19);
    let images = Images::new(&original)?;
    let mut c = images.session()?;
    expect_data(
        c.sector_io(7, 2, false, SectorOp::Read)?,
        &original[3584..4608],
    );
    let fresh = sector_data(2, 155);
    let old_cache = images.cache_bytes(0, 8192)?;
    let before = c.counts;
    let reply = c.sector_io_with_sync(7, 2, true, SectorOp::Read, |origin| {
        // This fixture-only mutation makes a read before the barrier distinguishable.
        let mut writer = images.open("origin.img", false, true)?;
        writer.seek(SeekFrom::Start(3584))?;
        writer.write_all(&fresh)?;
        origin.sync_all()
    })?;
    expect_data(reply, &fresh);
    assert_eq!(c.counts.origin_syncs - before.origin_syncs, 1);
    assert_eq!(c.counts.origin_reads - before.origin_reads, 1);
    assert_eq!(c.counts.cache_reads, before.cache_reads);
    assert!(images.cache_bytes(0, 8192)? == old_cache);
    Ok(())
}

#[test]
fn fua_write_sync_failure_keeps_new_bytes_and_rejects_the_old_fill() -> io::Result<()> {
    let original = sector_data(16, 61);
    let fresh = sector_data(2, 227);
    let images = Images::new(&original)?;
    let mut c = images.session()?;
    c.read_block(0)?;
    c.begin_fill(1)?;
    let error = c
        .sector_io_with_sync(7, 2, true, SectorOp::Write(&fresh), |_| {
            let bytes = fs::read(images.dir.join("origin.img"))?;
            assert!(bytes[3584..4608] == fresh);
            Err(io::Error::from_raw_os_error(1234))
        })
        .unwrap_err();
    assert_eq!(error.raw_os_error(), Some(1234));
    assert_eq!(c.complete_fill()?, Publication::Stale);
    let before = c.counts;
    expect_data(c.sector_io(7, 2, false, SectorOp::Read)?, &fresh);
    assert_eq!(c.counts.origin_reads - before.origin_reads, 2);
    assert_eq!(c.counts.origin_syncs, 1);
    Ok(())
}

#[test]
fn sync_errors_propagate_and_a_failed_write_never_reaches_sync() -> io::Result<()> {
    let original = sector_data(16, 41);
    let images = Images::new(&original)?;
    let mut c = images.session()?;
    for (op, fua) in [(SectorOp::Read, true), (SectorOp::Flush, false)] {
        let error = c
            .sector_io_with_sync(0, 1, fua, op, |_| Err(io::Error::from_raw_os_error(4321)))
            .unwrap_err();
        assert_eq!(error.raw_os_error(), Some(4321));
    }
    assert_eq!(
        (
            c.counts.origin_syncs,
            c.counts.origin_reads,
            c.counts.cache_reads
        ),
        (2, 0, 0)
    );
    c.origin = images.open("origin.img", true, false)?;
    let error = c
        .sector_io_with_sync(0, 1, true, SectorOp::Write(&[5; 512]), |_| {
            panic!("sync after failed write")
        })
        .unwrap_err();
    let direct = images
        .open("origin.img", true, false)?
        .write_all(&[5; 512])
        .unwrap_err();
    same_error(error, direct);
    assert_eq!(c.counts.origin_syncs, 2);
    assert!(fs::read(images.dir.join("origin.img"))? == original);
    Ok(())
}

#[test]
fn successful_fua_write_and_flush_use_the_origin_barrier() -> io::Result<()> {
    let original = sector_data(160, 37);
    let images = Images::new(&original)?;
    let mut c = images.session()?;
    assert_eq!(
        c.sector_io(159, 1, true, SectorOp::Write(&[7; 512]))?,
        SectorReply::Written
    );
    assert_eq!(
        c.sector_io(0, 0, false, SectorOp::Flush)?,
        SectorReply::Flushed
    );
    assert_eq!(
        c.sector_io(1, 159, false, SectorOp::Flush)?,
        SectorReply::Flushed
    );
    assert_eq!(c.counts.origin_syncs, 3);
    assert_eq!(
        c.sector_io(160, 0, false, SectorOp::Flush)
            .unwrap_err()
            .kind(),
        io::ErrorKind::InvalidInput
    );
    assert_eq!(c.counts.origin_syncs, 3);
    assert!(
        fs::read(images.dir.join("origin.img"))?[159 * 512..]
            .iter()
            .all(|&v| v == 7)
    );
    Ok(())
}

#[test]
fn a_short_origin_write_keeps_the_entire_requested_range_invalid() -> io::Result<()> {
    let mut expected = sector_data(16, 33);
    let fresh = sector_data(2, 191);
    let images = Images::new(&expected)?;
    let mut c = images.session()?;
    expect_data(
        c.sector_io(7, 2, false, SectorOp::Read)?,
        &expected[3584..4608],
    );
    let e = c
        .sector_io_with_origin(
            7,
            2,
            true,
            SectorOp::Write(&fresh),
            |file, data| {
                assert_eq!(data.len(), 1024);
                assert_eq!(file.stream_position()?, 3584);
                file.write_all(&data[..512])?;
                Err(io::Error::from_raw_os_error(1234))
            },
            File::sync_all,
        )
        .unwrap_err();
    assert_eq!(e.raw_os_error(), Some(1234));
    assert_eq!(c.counts.origin_syncs, 0);
    expected[3584..4096].copy_from_slice(&fresh[..512]);
    assert!(fs::read(images.dir.join("origin.img"))? == expected);
    let before = c.counts;
    expect_data(
        c.sector_io(7, 2, false, SectorOp::Read)?,
        &expected[3584..4608],
    );
    assert_eq!(
        (
            c.counts.origin_reads - before.origin_reads,
            c.counts.cache_reads - before.cache_reads
        ),
        (2, 0)
    );
    Ok(())
}

#[test]
fn both_warm_lines_are_revoked_before_the_lower_writer_can_return() -> io::Result<()> {
    let original = sector_data(16, 57);
    let images = Images::new(&original)?;
    let mut c = images.session()?;
    c.sector_io(7, 2, false, SectorOp::Read)?;
    assert_eq!(c.visible, [Some(0), Some(1)]);
    struct AtLowerWrite;
    // Stop inside the lower writer; any invalidation placed after it cannot run.
    let interrupted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        c.sector_io_with_origin(
            7,
            2,
            false,
            SectorOp::Write(&[42; 1024]),
            |file, bytes| {
                assert_eq!(file.stream_position()?, 3584);
                assert_eq!(bytes.len(), 1024);
                std::panic::panic_any(AtLowerWrite);
            },
            File::sync_all,
        )
    }));
    assert!(matches!(interrupted,Err(p) if p.is::<AtLowerWrite>()));
    assert_eq!(c.visible, [None, None]);
    assert_eq!(c.counts.origin_syncs, 0);
    assert!(fs::read(images.dir.join("origin.img"))? == original);
    Ok(())
}
