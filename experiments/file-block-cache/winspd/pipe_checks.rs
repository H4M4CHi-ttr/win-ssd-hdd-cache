//! WinSpd v1.0B1 x64 test messages. The native broker checks the layout against the pinned header.
use super::*;

const HEADER: usize = 48;
const MAX_MESSAGE: usize = HEADER + 65536;
const READ: u8 = 1;
const WRITE: u8 = 2;
const FLUSH: u8 = 3;
const UNMAP: u8 = 4;

fn invalid_message() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "invalid WinSpd test message")
}

struct Request<'a> {
    hint: u64,
    kind: u8,
    first: u64,
    count: u32,
    fua: bool,
    data: &'a [u8],
}

impl<'a> Request<'a> {
    fn decode(msg: &'a [u8]) -> io::Result<Self> {
        if !(HEADER..=MAX_MESSAGE).contains(&msg.len()) {
            return Err(invalid_message());
        }
        let word = |offset: usize| u32::from_le_bytes(msg[offset..offset + 4].try_into().unwrap());
        let quad = |offset: usize| u64::from_le_bytes(msg[offset..offset + 8].try_into().unwrap());
        let kind = msg[8];
        let data = &msg[HEADER..];
        let mut first = quad(16);
        let mut count = word(24);
        let mut fua = false;
        match kind {
            READ | WRITE => {
                let flags = word(28);
                if flags > 1
                    || (kind == READ && !data.is_empty())
                    || (kind == WRITE && data.len() as u64 != u64::from(count) * 512)
                {
                    return Err(invalid_message());
                }
                fua = flags == 1;
            }
            FLUSH if data.is_empty() => {}
            // The pinned client sends one descriptor. The backend always rejects Unmap.
            UNMAP if word(16) == 1 && data.len() == 16 => {
                first = u64::from_le_bytes(data[..8].try_into().unwrap());
                count = u32::from_le_bytes(data[8..12].try_into().unwrap());
            }
            _ => return Err(invalid_message()),
        }
        Ok(Self {
            hint: quad(0),
            kind,
            first,
            count,
            fua,
            data,
        })
    }

    fn op(&self) -> SectorOp<'_> {
        match self.kind {
            READ => SectorOp::Read,
            WRITE => SectorOp::Write(self.data),
            FLUSH => SectorOp::Flush,
            UNMAP => SectorOp::Unmap,
            _ => unreachable!("validated request kind"),
        }
    }

    fn response(&self, result: &io::Result<SectorReply>) -> io::Result<Vec<u8>> {
        let mut msg = vec![0; HEADER];
        msg[..8].copy_from_slice(&self.hint.to_le_bytes());
        msg[8] = self.kind;
        match result {
            Ok(SectorReply::Data(bytes))
                if self.kind == READ
                    && bytes.len() as u64 == u64::from(self.count) * 512
                    && bytes.len() <= 65536 =>
            {
                msg.extend_from_slice(bytes)
            }
            Ok(SectorReply::Written) if self.kind == WRITE => {}
            Ok(SectorReply::Flushed) if self.kind == FLUSH => {}
            Ok(_) => return Err(invalid_message()),
            Err(e) => {
                msg[16] = 2; // SCSI CHECK CONDITION; no successful read payload.
                let (key, asc) = match e.kind() {
                    io::ErrorKind::Unsupported => (5, 0x20),
                    io::ErrorKind::InvalidInput => (5, 0x24),
                    _ if self.kind == READ => (3, 0x11),
                    _ => (3, 0x0c),
                };
                msg[17] = key;
                msg[18] = asc;
            }
        }
        Ok(msg)
    }
}

#[test]
fn wire_rejects_truncated_padded_and_wrong_length_transfers() -> io::Result<()> {
    let mut msg = vec![0; HEADER];
    msg[0..8].copy_from_slice(&99_u64.to_le_bytes());
    msg[8] = READ;
    msg[16..24].copy_from_slice(&7_u64.to_le_bytes());
    msg[24..28].copy_from_slice(&2_u32.to_le_bytes());
    msg[28] = 1;
    assert!(Request::decode(&msg[..HEADER - 1]).is_err());
    let req = Request::decode(&msg)?;
    assert_eq!(
        (req.hint, req.kind, req.first, req.count, req.fua),
        (99, READ, 7, 2, true)
    );
    assert!(matches!(req.op(), SectorOp::Read));
    assert!(req.response(&Ok(SectorReply::Data(vec![1; 1023]))).is_err());
    let rsp = req.response(&Ok(SectorReply::Data(vec![1; 1024])))?;
    assert_eq!(rsp.len(), HEADER + 1024);
    assert_eq!(rsp[8], READ);
    assert!(rsp[HEADER..].iter().all(|&v| v == 1));
    msg.push(0);
    assert!(Request::decode(&msg).is_err());
    msg[8] = WRITE;
    assert!(Request::decode(&msg).is_err());
    msg.resize(HEADER + 1024, 42);
    assert!(matches!(Request::decode(&msg)?.op(),SectorOp::Write(buf) if buf.len()==1024));
    msg[28] = 2;
    assert!(Request::decode(&msg).is_err());
    msg[8] = UNMAP;
    msg[16..20].copy_from_slice(&1_u32.to_le_bytes());
    msg.truncate(HEADER + 16);
    let rejected = Request::decode(&msg)?.response(&Err(io::ErrorKind::Unsupported.into()))?;
    assert_eq!(rejected.len(), HEADER);
    assert_eq!(&rejected[16..19], &[2, 5, 0x20]);
    Ok(())
}

#[cfg(windows)]
mod live {
    use super::*;
    use std::process::{Child, Command, Stdio};

    const TEST: &str = "cache::tests::winspd::live::official_client_over_private_pipe";

    // Unwinding on any failed assertion also reaps the broker. Its private job owns stgtest.
    pub(super) struct OwnedChild(pub(super) Child);
    impl Drop for OwnedChild {
        fn drop(&mut self) {
            if self.0.try_wait().ok().flatten().is_none() {
                let _ = self.0.kill();
            }
            let _ = self.0.wait();
        }
    }

    pub(super) fn receive(input: &mut impl Read) -> io::Result<Vec<u8>> {
        let mut size = [0; 4];
        input.read_exact(&mut size)?;
        let n = u32::from_le_bytes(size) as usize;
        if !(HEADER..=MAX_MESSAGE).contains(&n) {
            return Err(invalid_message());
        }
        let mut msg = vec![0; n];
        input.read_exact(&mut msg)?;
        Ok(msg)
    }

    pub(super) fn send(output: &mut impl Write, msg: &[u8]) -> io::Result<()> {
        output.write_all(&(msg.len() as u32).to_le_bytes())?;
        output.write_all(msg)?;
        output.flush()
    }

    pub(super) fn start_broker(name: &str) -> io::Result<OwnedChild> {
        let exe = std::env::current_exe()?.with_file_name("file-cache-pipe-broker.exe");
        Ok(OwnedChild(
            Command::new(exe)
                .arg(name)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?,
        ))
    }

    fn scenario(name: &str, ops: &[u8], sdk: bool) -> io::Result<()> {
        let mut expected: Vec<u8> = (0..131 * 512).map(|i| (i % 251) as u8).collect();
        let images = Images::new(&expected)?;
        let mut c = images.session()?;
        if matches!(name, "read-error" | "flush-error") {
            c.origin = images.open("origin.img", true, false)?;
        }
        let direct_error = if matches!(
            name,
            "read-error" | "flush-error" | "read-after-write-error" | "close-error"
        ) {
            Some(
                images
                    .open("origin.img", true, false)?
                    .sync_all()
                    .expect_err("Windows read-only flush should fail"),
            )
        } else {
            None
        };
        let mut broker = start_broker(name)?;
        let mut input = broker.0.stdout.take().unwrap();
        let mut output = broker.0.stdin.take().unwrap();
        let mut hints = Vec::new();
        let mut error_responses = 0;
        for (i, &kind) in ops.iter().enumerate() {
            let bytes = receive(&mut input)?;
            let req = Request::decode(&bytes)?;
            let first = if name == "warm" && i >= 4 { 9 } else { 7 };
            let fua = matches!(kind, READ | WRITE)
                && matches!(
                    name,
                    "fua" | "write-error" | "read-error" | "read-after-write-error"
                );
            assert_eq!(
                (req.kind, req.first, req.count, req.fua),
                (kind, first, 2, fua)
            );
            assert_eq!(req.hint as u32, i as u32);
            assert!(!hints.contains(&req.hint));
            hints.push(req.hint);
            if name == "deadline" {
                // Deliberately keep the anonymous reply pipe open and supply no response.
                println!("WIRE {name} seq={i} op={kind} lba={first} count=2 withheld=true");
                break;
            }
            let offset = first as usize * 512;
            if name == "warm" && i % 4 == 2 {
                c.origin = images.open("origin.img", false, true)?;
            }
            if name == "warm" && i % 4 == 3 {
                c.origin = images.open("origin.img", true, true)?;
            }
            if name == "read-after-write-error" && kind == READ {
                c.origin = images.open("origin.img", true, false)?;
            }
            let prior = c.counts;
            let result = if name == "write-error" {
                c.sector_io_with_sync(first, 2, true, req.op(), |_| {
                    assert!(
                        fs::read(images.dir.join("origin.img"))?[offset..offset + 1024]
                            == *req.data
                    );
                    Err(io::Error::from_raw_os_error(1234)) // inject only after real origin write
                })
            } else {
                c.sector_io(req.first, req.count, req.fua, req.op())
            };
            let barrier = u64::from(fua || kind == FLUSH);
            assert_eq!(c.counts.origin_syncs - prior.origin_syncs, barrier);
            let mut reply = req.response(&result)?;
            let raw_error = result.as_ref().err().and_then(io::Error::raw_os_error);
            if kind == WRITE {
                expected[offset..offset + 1024].copy_from_slice(req.data);
            }
            if matches!(name, "write-error" | "read-error" | "flush-error" | "unmap")
                || (name == "read-after-write-error" && kind == READ)
            {
                let error = result.unwrap_err();
                if let Some(ref probe) = direct_error {
                    assert_eq!(
                        (error.kind(), error.raw_os_error()),
                        (probe.kind(), probe.raw_os_error())
                    );
                }
                if name == "write-error" {
                    assert_eq!(error.raw_os_error(), Some(1234));
                }
                if kind == UNMAP {
                    assert_eq!(error.kind(), io::ErrorKind::Unsupported);
                }
                let sense = match kind {
                    READ => [2, 3, 0x11],
                    UNMAP => [2, 5, 0x20],
                    _ => [2, 3, 0x0c],
                };
                assert_eq!(&reply[16..19], &sense);
                assert_eq!(reply.len(), HEADER);
                error_responses += 1;
            } else {
                assert!(result.is_ok());
                assert_eq!(reply[16], 0);
                if kind == READ {
                    assert!(reply[HEADER..] == expected[offset..offset + 1024]);
                    if name == "fua" {
                        assert_eq!(c.counts.origin_reads - prior.origin_reads, 1);
                        assert_eq!(c.counts.cache_reads, prior.cache_reads);
                    } else if i % 4 == 2 {
                        assert_eq!(c.counts.origin_reads, prior.origin_reads);
                        assert_eq!(
                            c.counts.cache_reads - prior.cache_reads,
                            if first == 7 { 2 } else { 1 }
                        );
                    }
                } else {
                    assert_eq!(reply.len(), HEADER);
                }
            }
            if name == "corrupt-reply" && kind == READ {
                // Negative control: the genuine client must detect a bad byte despite GOOD.
                reply[HEADER + 19] ^= 1;
            }
            send(&mut output, &reply)?;
            println!(
                "WIRE {name} seq={i} op={kind} lba={first} count=2 fua={fua} scsi={} sense={:02x}/{:02x} bytes={} sync_attempts={barrier} raw_error={raw_error:?}",
                reply[16],
                reply[17],
                reply[18],
                reply.len() - HEADER
            );
        }
        let external_syncs = c.counts.origin_syncs;
        let close_expected = sdk
            && matches!(
                name,
                "warm" | "unmap" | "flush-error" | "corrupt-reply" | "close-error"
            );
        let mut close_error = 0_u32;
        if close_expected {
            let mut tag = [0; 4];
            input.read_exact(&mut tag)?;
            assert_eq!(
                u32::from_le_bytes(tag),
                0x464c5348,
                "missing DLL shutdown Flush"
            );
            if name == "close-error" {
                c.origin = images.open("origin.img", true, false)?;
            }
            if name == "flush-error" {
                c.origin = images.open("origin.img", true, true)?;
            }
            let result = c.sector_io(0, 0, false, SectorOp::Flush);
            if name == "close-error" {
                let e = result.unwrap_err();
                let probe = direct_error.as_ref().unwrap();
                assert_eq!(
                    (e.kind(), e.raw_os_error()),
                    (probe.kind(), probe.raw_os_error())
                );
                close_error = u32::try_from(e.raw_os_error().unwrap()).unwrap();
                assert_ne!(close_error, 0);
            } else {
                assert_eq!(result?, SectorReply::Flushed);
            }
            output.write_all(&close_error.to_le_bytes())?;
            output.flush()?;
            assert_eq!(c.counts.origin_syncs - external_syncs, 1);
        }
        let status = broker.0.wait()?;
        drop(output);
        let mut logs = String::new();
        broker.0.stderr.take().unwrap().read_to_string(&mut logs)?;
        let mut surplus = Vec::new();
        input.read_to_end(&mut surplus)?;
        assert!(surplus.is_empty(), "unexpected extra request");
        assert!(fs::read(images.dir.join("origin.img"))? == expected);
        if name == "deadline" {
            assert_eq!(status.code(), Some(1460));
            assert!(
                logs.contains("BROKER deadline reaped=1 stage=reply"),
                "{logs}"
            );
        } else {
            let expected_exit = if close_error != 0 {
                29
            } else if name == "callback-mismatch" {
                13
            } else {
                0
            };
            assert_eq!(status.code(), Some(expected_exit), "broker {name}: {logs}");
            assert!(logs.contains("reaped=1"), "{logs}");
            if name == "peer-check" {
                assert!(
                    logs.contains("peer-rejected before-geometry")
                        && !logs.contains("peer-verified")
                );
            } else if name == "inner-busy" {
                assert!(
                    logs.contains("SDK inner-busy before-dispatch") && !logs.contains("SDK ready"),
                    "{logs}"
                );
            } else {
                assert!(logs.contains("BROKER peer-verified"));
                let client_exit = if error_responses == 0
                    && !matches!(name, "corrupt-reply" | "callback-mismatch")
                {
                    0
                } else {
                    1117
                };
                assert!(
                    logs.contains(&format!("client-result={client_exit} reaped=1")),
                    "{logs}"
                );
            }
        }
        if matches!(
            name,
            "fua"
                | "read-error"
                | "flush-error"
                | "write-error"
                | "unmap"
                | "peer-check"
                | "deadline"
                | "callback-mismatch"
                | "inner-busy"
                | "read-after-write-error"
        ) {
            assert!(images.cache_bytes(0, 8192)?.iter().all(|&v| v == 0));
        }
        if sdk {
            let client_requests = logs
                .lines()
                .filter(|l| l.starts_with("SDK outer-request "))
                .count();
            let mismatches = usize::from(name == "callback-mismatch");
            assert_eq!(client_requests, hints.len() + mismatches, "{logs}");
            if !matches!(name, "peer-check" | "inner-busy") {
                assert!(
                    logs.contains("SDK ready inner-peer=verified dispatchers=1"),
                    "{name}: {logs}"
                );
                let callbacks = logs
                    .lines()
                    .filter(|s| s.starts_with("SDK callback "))
                    .count();
                assert_eq!(callbacks, hints.len(), "{logs}");
                if name != "deadline" {
                    let rejected = u32::from(name == "callback-mismatch");
                    assert!(logs.contains(&format!("SDK stopped callbacks={callbacks} rejected={rejected} close_calls={} close_error={close_error} waited=1",u32::from(close_expected))),"{logs}");
                }
                if matches!(name, "read-error" | "read-after-write-error") {
                    assert!(
                        logs.contains(
                            "SDK failed-read internal_bytes=1024 outer_bytes=0 zero_checked=1"
                        ),
                        "{logs}"
                    );
                }
            } else {
                assert!(!logs.contains("SDK ready") && !logs.contains("SDK callback"));
            }
            for line in logs
                .lines()
                .filter(|s| s.starts_with("SDK ") || s.starts_with("BROKER client-result="))
            {
                println!("DLL {name} {line}");
            }
            println!(
                "SERVER_DLL {name} client_requests={client_requests} forwarded_to_rust={} rejected_callbacks={mismatches} external_syncs={external_syncs} close_calls={} close_error={close_error} broker_exit={}",
                hints.len(),
                u32::from(close_expected),
                status.code().unwrap()
            );
        }
        println!(
            "WINSPD {name} requests={} sync={} origin_reads={} cache_reads={} check_conditions={} client_reaped=true",
            hints.len(),
            c.counts.origin_syncs,
            c.counts.origin_reads,
            c.counts.cache_reads,
            error_responses
        );
        Ok(())
    }

    fn exercise(sdk: bool) -> io::Result<()> {
        let test = if sdk {
            "cache::tests::winspd::live::official_server_dll_over_private_pipe"
        } else {
            TEST
        };
        if !std::env::args()
            .skip(1)
            .eq(["--ignored", "--exact", test, "--nocapture"])
        {
            return Ok(());
        }
        assert_eq!(
            std::env::current_exe()?.file_name().unwrap(),
            "file-cache-pipe-test.exe",
            "run the isolated winspd/run.ps1 launcher"
        );
        for (name, ops) in [
            (
                "warm",
                &[WRITE, READ, READ, FLUSH, WRITE, READ, READ, FLUSH][..],
            ),
            ("fua", &[WRITE, READ, FLUSH]),
            ("write-error", &[WRITE]),
            ("read-error", &[READ]),
            ("flush-error", &[FLUSH]),
            ("unmap", &[UNMAP]),
            ("corrupt-reply", &[WRITE, READ]),
            ("peer-check", &[]),
            ("deadline", &[READ]),
        ] {
            scenario(name, ops, sdk)?;
        }
        if sdk {
            for (name, ops) in [
                ("read-after-write-error", &[WRITE, READ][..]),
                ("close-error", &[WRITE, READ]),
                ("inner-busy", &[]),
                ("callback-mismatch", &[]),
            ] {
                scenario(name, ops, true)?;
            }
            println!("FILE_CACHE_WINSPD_SERVER_DLL_OK");
        } else {
            println!("FILE_CACHE_WINSPD_OK");
        }
        Ok(())
    }

    #[test]
    #[ignore = "the Windows runner supplies the pinned client"]
    fn official_client_over_private_pipe() -> io::Result<()> {
        exercise(false)
    }

    #[test]
    #[ignore = "the Windows runner supplies the pinned client and server DLL"]
    fn official_server_dll_over_private_pipe() -> io::Result<()> {
        exercise(true)
    }
}

#[cfg(windows)]
#[path = "recovery_checks.rs"]
mod recovery;

#[cfg(windows)]
#[path = "readonly_checks.rs"]
mod read_only;
