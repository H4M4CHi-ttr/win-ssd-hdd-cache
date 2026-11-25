//! Counts backing-file reads while comparing every returned byte against an independent image.
use super::*;

const LINE: usize = 4096;

#[derive(Clone, Copy)]
enum Step {
    Read(usize),
    CrossLineWrite(u8),
    Reopen,
}

#[derive(Default)]
struct Totals {
    reads: usize,
    writes: usize,
    reopens: usize,
    origin_reads: usize,
    cache_reads: usize,
}

enum Backend {
    Origin(File),
    Cached(FileCache),
}

impl Backend {
    fn open(images: &Images, cached: bool) -> io::Result<Self> {
        if cached {
            images.session().map(Self::Cached)
        } else {
            images.open("origin.img", true, true).map(Self::Origin)
        }
    }

    fn read(&mut self, line: usize, totals: &mut Totals) -> io::Result<Vec<u8>> {
        let offset = (line * LINE) as u64;
        totals.reads += 1;
        match self {
            Self::Origin(file) => {
                file.seek(SeekFrom::Start(offset))?;
                let mut bytes = vec![0; LINE];
                file.read_exact(&mut bytes)?;
                totals.origin_reads += 1;
                Ok(bytes)
            }
            Self::Cached(cache) => {
                // All reads end on a line boundary. Cursor 1 is an independent witness
                // that an origin read was avoided. Seeking excludes timing comparisons.
                cache.origin.seek(SeekFrom::Start(1))?;
                cache.cache.seek(SeekFrom::Start(1))?;
                let before = cache.counts;
                let SectorReply::Data(bytes) =
                    cache.sector_io((line * 8) as u64, 8, false, SectorOp::Read)?
                else {
                    panic!("read did not return data");
                };
                let end = cache.origin.stream_position()?;
                let reads = (
                    cache.counts.origin_reads - before.origin_reads,
                    cache.counts.cache_reads - before.cache_reads,
                );
                if end == 1 {
                    assert_eq!(reads, (0, 1));
                    assert_eq!(
                        cache.cache.stream_position()?,
                        ((line % 2 + 1) * LINE) as u64
                    );
                    totals.cache_reads += 1;
                } else {
                    assert_eq!(end, offset + LINE as u64);
                    assert_eq!(reads, (1, 0));
                    totals.origin_reads += 1;
                }
                Ok(bytes)
            }
        }
    }

    fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        match self {
            Self::Origin(file) => {
                file.seek(SeekFrom::Start(3584))?;
                file.write_all(bytes)
            }
            Self::Cached(cache) => {
                assert_eq!(
                    cache.sector_io(7, 2, false, SectorOp::Write(bytes))?,
                    SectorReply::Written
                );
                Ok(())
            }
        }
    }
}

fn replay(steps: &[Step], cached: bool) -> io::Result<(Totals, Vec<u8>)> {
    let mut expected: Vec<u8> = (0..3 * LINE)
        .map(|i| ((i * 13 + i / LINE * 29) % 251) as u8)
        .collect();
    let images = Images::new(&expected)?;
    let mut backend = Backend::open(&images, cached)?;
    let mut totals = Totals::default();
    for (index, step) in steps.iter().enumerate() {
        match *step {
            Step::Read(line) => {
                let bytes = backend.read(line, &mut totals)?;
                assert!(
                    bytes == expected[line * LINE..(line + 1) * LINE],
                    "incorrect bytes at step {index}, line {line}"
                );
            }
            Step::CrossLineWrite(version) => {
                let patch: Vec<u8> = (0..1024)
                    .map(|i| version.wrapping_add((i * 11 + i / 97) as u8))
                    .collect();
                backend.write(&patch)?;
                expected[3584..4608].copy_from_slice(&patch);
                totals.writes += 1;
            }
            Step::Reopen => {
                drop(backend);
                backend = Backend::open(&images, cached)?;
                totals.reopens += 1;
            }
        }
    }
    let physical = fs::read(images.dir.join("origin.img"))?;
    assert!(
        physical == expected,
        "final origin differs from reference bytes"
    );
    Ok((totals, physical))
}

fn compare(
    label: &str,
    steps: &[Step],
    misses: usize,
    hits: usize,
    writes: usize,
    reopens: usize,
) -> io::Result<()> {
    let (base, base_bytes) = replay(steps, false)?;
    let (cached, cached_bytes) = replay(steps, true)?;
    assert!(cached_bytes == base_bytes);
    assert_eq!(
        (base.reads, base.origin_reads, base.cache_reads),
        (1024, 1024, 0)
    );
    assert_eq!(
        (cached.reads, cached.origin_reads, cached.cache_reads),
        (1024, misses, hits)
    );
    assert_eq!(
        (base.writes, cached.writes, base.reopens, cached.reopens),
        (writes, writes, reopens, reopens)
    );
    println!(
        "FILE_WORKLOAD {label} reads=1024 writes={writes} reopens={reopens} baseline_origin={} cached_origin={} cache_hits={} bytes_checked={}",
        base.origin_reads,
        cached.origin_reads,
        cached.cache_reads,
        cached.reads * LINE
    );
    Ok(())
}

#[test]
fn two_resident_lines_need_only_two_origin_reads() -> io::Result<()> {
    let steps: Vec<_> = (0..1024).map(|i| Step::Read(i % 2)).collect();
    compare("two-resident", &steps, 2, 1022, 0, 0)
}

#[test]
fn two_conflicting_lines_have_no_cache_hits() -> io::Result<()> {
    let steps: Vec<_> = (0..1024).map(|i| Step::Read((i % 2) * 2)).collect();
    compare("same-slot", &steps, 1024, 0, 0, 0)
}

#[test]
fn repeated_cross_line_writes_and_reopens_preserve_every_version() -> io::Result<()> {
    let mut steps = Vec::new();
    for version in 0..128 {
        if version != 0 && version % 16 == 0 {
            steps.push(Step::Reopen);
        }
        steps.extend([
            Step::Read(0),
            Step::Read(1),
            Step::CrossLineWrite(version),
            Step::Read(0),
            Step::Read(1),
            Step::Read(0),
            Step::Read(1),
            Step::Read(2),
            Step::Read(0),
        ]);
    }
    // Four compulsory origin reads per round after writes and slot replacement,
    // plus two cold reads in each of eight sessions.
    compare("write-reopen", &steps, 4 * 128 + 2 * 8, 496, 128, 7)
}
