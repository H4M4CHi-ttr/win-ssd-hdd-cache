use super::*;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

struct Worker(Child);
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
struct PrivateFiles(PathBuf);
impl Drop for PrivateFiles {
    fn drop(&mut self) {
        for file in ["journal", "state", "ready"] {
            let _ = fs::remove_file(self.0.join(file));
        }
        let _ = fs::remove_dir(&self.0);
    }
}
fn durable(path: &Path, bytes: &[u8]) {
    let mut file = File::create(path).unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
}

// This encoder inspects the store, never the executor's journal/call log.
fn state(inventory: &Inventory) -> [u8; 4] {
    let (_, _, package) = fixture();
    let p = u8::from(inventory.package == Observed::Known(Some(package)));
    let s = u8::from(
        inventory.service
            == ServiceState::DemandStopped {
                name: "Probe".into(),
                image_hash: [3; 32],
                owner_attempt: 42,
            },
    );
    let k =
        u8::from(inventory.selector == RegistryRead::Value(identity("T", 1).instance.selector()));
    let f = u8::from(
        inventory
            .disks
            .iter()
            .find(|d| d.object == T)
            .unwrap()
            .upper
            == FilterValue::Names(vec!["KeepA".into(), "keepB".into(), "Probe".into()]).registry(),
    );
    [p, s, k, f]
}
fn fresh_enumeration(bytes: &[u8]) -> Inventory {
    assert_eq!(bytes.len(), 4);
    let (mut inventory, _, package) = fixture();
    if bytes[0] == 1 {
        inventory.package = Observed::Known(Some(package));
    }
    if bytes[1] == 1 {
        inventory.service = ServiceState::DemandStopped {
            name: "Probe".into(),
            image_hash: [3; 32],
            owner_attempt: 42,
        };
    }
    if bytes[2] == 1 {
        inventory.selector = RegistryRead::Value(identity("T", 1).instance.selector());
    }
    if bytes[3] == 1 {
        target(&mut inventory).upper =
            FilterValue::Names(vec!["KeepA".into(), "keepB".into(), "Probe".into()]).registry();
    }
    inventory
}

fn crash(
    case: &str,
    cutpoint: Checkpoint,
    expected_state: [u8; 4],
    expected_events: Vec<JournalEvent>,
) {
    if std::env::var("P04_SYNTHETIC_CRASH_CASE").as_deref() == Ok(case) {
        let directory = PathBuf::from(std::env::var_os("P04_SYNTHETIC_CRASH_DIRECTORY").unwrap());
        let (plan, mut lab) = plan();
        lab.apply_observed(&plan, |lab, point| {
            if point == cutpoint {
                oracle(lab, expected_state.iter().filter(|b| **b == 1).count());
                durable(
                    &directory.join("journal"),
                    &lab.journal.as_ref().unwrap().to_private_bytes(),
                );
                durable(&directory.join("state"), &state(&lab.inventory));
                durable(&directory.join("ready"), b"isolated-fake-at-cutpoint");
                loop {
                    std::thread::park();
                }
            }
        });
        panic!("fake failed to reach cutpoint");
    }
    let directory =
        std::env::temp_dir().join(format!("p04-reg-fake-{}-{case}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    let files = PrivateFiles(directory);
    let mut worker = Worker(
        Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                &format!("tests::crash_tests::{case}"),
                "--nocapture",
            ])
            .env("P04_SYNTHETIC_CRASH_CASE", case)
            .env("P04_SYNTHETIC_CRASH_DIRECTORY", &files.0)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    while !files.0.join("ready").is_file() {
        assert!(
            worker.0.try_wait().unwrap().is_none(),
            "isolated fake exited before cutpoint"
        );
        assert!(
            Instant::now() < deadline,
            "test harness failed to reach fake cutpoint"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    worker.0.kill().unwrap();
    let status = worker.0.wait().unwrap();
    assert!(!status.success());
    let journal_bytes = fs::read(files.0.join("journal")).unwrap();
    let actual_state = fs::read(files.0.join("state")).unwrap();
    assert_eq!(actual_state, expected_state);
    let (plan, _) = plan();
    let journal = Journal::from_private_bytes(&plan, &journal_bytes).unwrap();
    assert_eq!(journal.events(), expected_events);
    let mut restarted =
        SyntheticLab::restart(fresh_enumeration(&actual_state), fixture().1, Some(journal));
    let result = restarted.apply(&plan);
    assert_eq!(result.classification, Classification::Unknown);
    assert!(restarted.calls.is_empty() && restarted.changes.is_empty());
    assert_eq!(
        result.observed_effects,
        MUTATION_STAGES[..expected_state.iter().filter(|b| **b == 1).count()]
    );
    // Lost private record is not a first attempt, even when the store resembles completion.
    restarted.journal = None;
    assert_eq!(
        restarted.restart_read_only(&plan).classification,
        Classification::Unknown
    );
    assert!(restarted.calls.is_empty() && restarted.changes.is_empty());
}

fn events(prefix: usize, intent: Option<Stage>) -> Vec<JournalEvent> {
    let mut events = vec![];
    for stage in [Stage::P, Stage::S, Stage::K, Stage::F][..prefix]
        .iter()
        .copied()
    {
        events.extend([
            JournalEvent::Intent(stage),
            JournalEvent::Returned(stage, true),
            JournalEvent::Confirmed(stage),
        ]);
    }
    if let Some(stage) = intent {
        events.push(JournalEvent::Intent(stage));
    }
    events
}
macro_rules! cutpoint {
    ($name:ident,$point:expr,$state:expr,$events:expr) => {
        #[test]
        fn $name() {
            crash(stringify!($name), $point, $state, $events);
        }
    };
}
cutpoint!(
    r13b_p_before_process_kill,
    Checkpoint::IntentSaved(Stage::P),
    [0, 0, 0, 0],
    events(0, Some(Stage::P))
);
cutpoint!(
    r13b_p_after_process_kill,
    Checkpoint::EffectApplied(Stage::P),
    [1, 0, 0, 0],
    events(0, Some(Stage::P))
);
cutpoint!(
    r13b_s_before_process_kill,
    Checkpoint::IntentSaved(Stage::S),
    [1, 0, 0, 0],
    events(1, Some(Stage::S))
);
cutpoint!(
    r13b_s_after_process_kill,
    Checkpoint::EffectApplied(Stage::S),
    [1, 1, 0, 0],
    events(1, Some(Stage::S))
);
cutpoint!(
    r13b_k_before_process_kill,
    Checkpoint::IntentSaved(Stage::K),
    [1, 1, 0, 0],
    events(2, Some(Stage::K))
);
cutpoint!(
    r13b_k_after_process_kill,
    Checkpoint::EffectApplied(Stage::K),
    [1, 1, 1, 0],
    events(2, Some(Stage::K))
);
cutpoint!(
    r13b_f_before_process_kill,
    Checkpoint::IntentSaved(Stage::F),
    [1, 1, 1, 0],
    events(3, Some(Stage::F))
);
cutpoint!(
    r13b_f_after_process_kill,
    Checkpoint::EffectApplied(Stage::F),
    [1, 1, 1, 1],
    events(3, Some(Stage::F))
);
cutpoint!(
    r14_v_process_kill,
    Checkpoint::VerificationStarted,
    [1, 1, 1, 1],
    events(4, None)
);
cutpoint!(
    r14_before_complete_process_kill,
    Checkpoint::BeforeCompletion,
    [1, 1, 1, 1],
    events(4, None)
);
cutpoint!(
    r14_mid_verification_process_kill,
    Checkpoint::VerificationObserved,
    [1, 1, 1, 1],
    events(4, None)
);

#[test]
fn r14_private_record_rejects_truncation_and_other_identity_binding() {
    let (plan, mut lab) = plan();
    lab.apply(&plan);
    let bytes = lab.journal.as_ref().unwrap().to_private_bytes();
    for end in [0, 7, 15, 16, bytes.len() - 1] {
        assert!(Journal::from_private_bytes(&plan, &bytes[..end]).is_err());
    }
    let (mut inventory, mut protection, package) = fixture();
    target(&mut inventory).identity.serial = "new-private-serial".into();
    protection.target.serial = "new-private-serial".into();
    let other = make_plan(42, &inventory, &protection, &package, &package).unwrap();
    assert_eq!(
        Journal::from_private_bytes(&other, &bytes),
        Err(Reason::StateChanged)
    );
}
