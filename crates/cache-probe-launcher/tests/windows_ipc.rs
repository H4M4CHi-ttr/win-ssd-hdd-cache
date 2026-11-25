#![cfg(all(windows, target_arch = "x86_64", feature = "synthetic-ipc"))]
use cache_probe_launcher::{Child, Signal, Synthetic, launch_synthetic};
use std::{
    io::{BufRead, BufReader, Read, Write},
    sync::mpsc::{self, Receiver},
    time::Duration,
};
struct Running {
    child: Child,
    lines: Receiver<String>,
    bytes: Receiver<Vec<u8>>,
    ended: bool,
}
impl Running {
    fn new(kind: Synthetic) -> (Self, Signal) {
        let (child, signal) = launch_synthetic(kind).expect("synthetic child start");
        (Self::attach(child), signal)
    }
    fn attach(child: Child) -> Self {
        let reader = child.diagnostics.try_clone().unwrap();
        let output = child.output.try_clone().unwrap();
        let (tx, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(reader).lines() {
                if tx.send(line.unwrap()).is_err() {
                    break;
                }
            }
        });
        let (tx, bytes) = mpsc::channel();
        std::thread::spawn(move || {
            let mut output = output;
            let mut bytes = Vec::new();
            output.read_to_end(&mut bytes).unwrap();
            let _ = tx.send(bytes);
        });
        Self {
            child,
            lines,
            bytes,
            ended: false,
        }
    }
    fn line(&self, wanted: &str) {
        let line = self
            .lines
            .recv_timeout(Duration::from_secs(10))
            .unwrap_or_else(|_| {
                panic!(
                    "worker diagnostic unavailable; process status {:?}",
                    self.child.wait(0)
                )
            });
        assert_eq!(line, wanted);
    }
    fn release(&mut self, notified: bool) {
        self.child
            .input
            .write_all(if notified { b"A" } else { b"N" })
            .unwrap()
    }
    fn finish(&mut self, valid: bool) {
        let status = self.child.wait(10000).unwrap();
        if status.is_none() {
            let _ = self.child.terminate_synthetic();
            panic!("worker hang cleanup; test failed")
        }
        self.ended = true;
        assert_eq!(status, Some(0));
        assert_eq!(
            self.bytes.recv_timeout(Duration::from_secs(5)).unwrap(),
            if valid {
                vec![0, 255, 13, 10, 80, 48, 52]
            } else {
                vec![]
            }
        );
    }
}
impl Drop for Running {
    fn drop(&mut self) {
        if !self.ended {
            let _ = self.child.terminate_synthetic();
        }
    }
}
#[test]
fn c02_b03_real_windows_event_type_rights_and_invalid_startup() {
    for kind in [
        Synthetic::ExcessRights,
        Synthetic::WrongType,
        Synthetic::Stale,
        Synthetic::Missing,
        Synthetic::Zero,
        Synthetic::InvalidDigits,
        Synthetic::Width,
        Synthetic::Pseudo,
        Synthetic::NoPipe,
        Synthetic::NonInheritableDecoy,
    ] {
        let (mut child, _) = Running::new(kind);
        child.line("REFUSED");
        child.finish(false);
    }
}
#[test]
fn c03_b03_distinct_attempts_duplicate_notification_and_binary_pipes() {
    let (mut a, signal_a) = Running::new(Synthetic::Normal);
    let (mut b, _signal_b) = Running::new(Synthetic::Normal);
    assert_eq!(a.child.trial(), signal_a.trial());
    assert_ne!(a.child.trial(), b.child.trial());
    a.line("READY");
    b.line("READY");
    signal_a.notify().unwrap();
    signal_a.notify().unwrap();
    // Worker remains blocked on its independent pipe barrier, despite notification.
    assert_eq!(a.child.wait(0).unwrap(), None);
    a.release(true);
    b.release(false);
    a.line("ACCEPTED");
    a.line("CLOSED");
    b.line("NOT_NOTIFIED");
    b.line("CLOSED");
    a.finish(true);
    b.finish(true);
}
#[test]
fn c03_b03_sender_loss_does_not_signal_or_acknowledge() {
    let (mut child, signal) = Running::new(Synthetic::Normal);
    child.line("READY");
    let raw = signal.synthetic_raw();
    drop(signal);
    assert!(cache_probe_launcher::synthetic_not_inherited(raw));
    child.release(false);
    child.line("NOT_NOTIFIED");
    child.line("CLOSED");
    child.finish(true);
}

#[test]
fn c03_b03_sender_process_exits_without_notification() {
    let (mut sender, _outer_signal) =
        launch_synthetic(Synthetic::SenderExit).expect("synthetic sender start");
    if sender.wait(10000).unwrap() != Some(0) {
        let _ = sender.terminate_synthetic();
        panic!("sender process did not exit cleanly")
    }
    let mut packet = Vec::new();
    sender.output.read_to_end(&mut packet).unwrap();
    // SAFETY: exact private packet from our fixed, pinned synthetic sender,
    // addressed to this test process; fresh transferred handles adopted once.
    let child = unsafe { cache_probe_launcher::import_synthetic_child(&packet) }.unwrap();
    let mut child = Running::attach(child);
    child.line("READY");
    child.release(false);
    child.line("NOT_NOTIFIED");
    child.line("CLOSED");
    child.finish(true);
}
