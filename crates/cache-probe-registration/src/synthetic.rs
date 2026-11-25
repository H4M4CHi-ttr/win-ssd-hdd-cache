//! Concrete, memory-only registration lab. This module cannot call a Windows backend.

use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stage {
    P,
    S,
    K,
    F,
    V,
}
pub const MUTATION_STAGES: [Stage; 4] = [Stage::P, Stage::S, Stage::K, Stage::F];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Target {
    Package,
    Service,
    Selector,
    Devnode(u64),
    Class,
    Lower(u64),
    Function(u64),
    Vm,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Actor {
    Registrar,
    Competitor,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Mutation {
    pub stage: Stage,
    pub target: Target,
    pub actor: Actor,
}

// Requests cross the algorithm/fake-store boundary. The store derives its logs
// from the received request and performs its own target lookup, never plan labels.
pub(super) enum StoreRequest {
    Package(PackageManifest),
    Service(ServiceState),
    Selector(RegistryRead),
    Upper {
        object: u64,
        generation: u64,
        value: RegistryRead,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JournalEvent {
    Intent(Stage),
    Returned(Stage, bool),
    Confirmed(Stage),
    Complete,
}

#[derive(Clone, Eq, PartialEq)]
pub struct Journal {
    bound: Plan,
    events: Vec<JournalEvent>,
}
impl fmt::Debug for Journal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Journal")
            .field("events", &self.events)
            .finish()
    }
}
impl Journal {
    pub fn events(&self) -> &[JournalEvent] {
        &self.events
    }
    pub fn without_completion(&self) -> Self {
        Self {
            bound: self.bound.clone(),
            events: self
                .events
                .iter()
                .copied()
                .filter(|e| *e != JournalEvent::Complete)
                .collect(),
        }
    }

    /// Private persistence only: the binding contains raw identity evidence.
    pub fn to_private_bytes(&self) -> Vec<u8> {
        let binding = private_binding(&self.bound);
        let mut bytes = b"P04REG1\0".to_vec();
        bytes.extend_from_slice(&(binding.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&binding);
        for event in &self.events {
            let (kind, stage) = match event {
                JournalEvent::Intent(s) => (1, *s),
                JournalEvent::Returned(s, false) => (2, *s),
                JournalEvent::Returned(s, true) => (3, *s),
                JournalEvent::Confirmed(s) => (4, *s),
                JournalEvent::Complete => (5, Stage::V),
            };
            bytes.extend_from_slice(&[kind, stage_number(stage)]);
        }
        bytes
    }

    /// Restores only the exact, independently supplied immutable plan; malformed,
    /// truncated or another attempt's records remain unknown and cannot be resumed.
    pub fn from_private_bytes(plan: &Plan, bytes: &[u8]) -> Result<Self, Reason> {
        if bytes.len() < 16 || &bytes[..8] != b"P04REG1\0" {
            return Err(Reason::IncompleteAttempt);
        }
        let size = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
        let binding = private_binding(plan);
        if size != binding.len() as u64
            || bytes.get(16..16 + binding.len()) != Some(binding.as_slice())
        {
            return Err(Reason::StateChanged);
        }
        let tail = &bytes[16 + binding.len()..];
        let (pairs, remainder) = tail.as_chunks::<2>();
        if !remainder.is_empty() {
            return Err(Reason::IncompleteAttempt);
        }
        let mut events = vec![];
        for pair in pairs {
            let stage = match pair[1] {
                0 => Stage::P,
                1 => Stage::S,
                2 => Stage::K,
                3 => Stage::F,
                4 => Stage::V,
                _ => return Err(Reason::IncompleteAttempt),
            };
            let event = match pair[0] {
                1 if stage != Stage::V => JournalEvent::Intent(stage),
                2 if stage != Stage::V => JournalEvent::Returned(stage, false),
                3 if stage != Stage::V => JournalEvent::Returned(stage, true),
                4 if stage != Stage::V => JournalEvent::Confirmed(stage),
                5 if stage == Stage::V => JournalEvent::Complete,
                _ => return Err(Reason::IncompleteAttempt),
            };
            events.push(event);
        }
        Ok(Self {
            bound: plan.clone(),
            events,
        })
    }
}

fn stage_number(stage: Stage) -> u8 {
    match stage {
        Stage::P => 0,
        Stage::S => 1,
        Stage::K => 2,
        Stage::F => 3,
        Stage::V => 4,
    }
}

// Exact private binding, deliberately not a lossy hash or the redacted Debug view.
fn private_binding(plan: &Plan) -> Vec<u8> {
    fn add(bytes: &mut Vec<u8>, value: impl fmt::Debug) {
        let text = format!("{value:?}");
        bytes.extend_from_slice(&(text.len() as u64).to_le_bytes());
        bytes.extend_from_slice(text.as_bytes());
    }
    fn context(bytes: &mut Vec<u8>, c: &Context) {
        add(bytes, (&c.vm, &c.build, &c.snapshot));
    }
    fn identity(bytes: &mut Vec<u8>, i: &Identity) {
        add(
            bytes,
            (
                i.instance.as_str(),
                &i.storage_id,
                &i.serial,
                &i.parent_location,
                i.lun,
                &i.host_chain,
            ),
        );
    }
    fn property(bytes: &mut Vec<u8>, r: &RegistryRead) {
        match r {
            RegistryRead::Missing => add(bytes, 0),
            RegistryRead::Unavailable => add(bytes, 1),
            RegistryRead::Value(v) => {
                add(bytes, 2);
                add(bytes, (v.value_type, &v.bytes));
            }
        }
    }
    let mut bytes = vec![];
    add(
        &mut bytes,
        (plan.attempt, plan.target_object, plan.target_generation),
    );
    match &plan.before.context {
        Observed::Known(c) => {
            add(&mut bytes, 1);
            context(&mut bytes, c);
        }
        Observed::Unknown => add(&mut bytes, 0),
    }
    add(&mut bytes, plan.before.disks.len());
    for d in &plan.before.disks {
        add(
            &mut bytes,
            (
                d.object,
                d.generation,
                d.number,
                d.present,
                d.raw,
                d.partitions,
            ),
        );
        add(
            &mut bytes,
            (
                &d.capacity_bytes,
                &d.hardware_ids,
                &d.compatible_ids,
                &d.location_paths,
                &d.label,
            ),
        );
        identity(&mut bytes, &d.identity);
        add(
            &mut bytes,
            (&d.uses, &d.class, &d.stack, &d.function_service),
        );
        property(&mut bytes, &d.upper);
        property(&mut bytes, &d.lower);
    }
    property(&mut bytes, &plan.before.class_upper);
    property(&mut bytes, &plan.before.class_lower);
    add(&mut bytes, (&plan.before.package, &plan.before.service));
    property(&mut bytes, &plan.before.selector);
    context(&mut bytes, &plan.protection.context);
    identity(&mut bytes, &plan.protection.os);
    identity(&mut bytes, &plan.protection.target);
    add(
        &mut bytes,
        (
            &plan.protection.os_uses,
            &plan.protection.expected_class,
            &plan.protection.expected_stack,
        ),
    );
    add(&mut bytes, &plan.package);
    property(&mut bytes, &plan.after_upper);
    add(&mut bytes, &plan.os_usage_changes);
    bytes
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Exclusion {
    WholeLab { epoch: u64 },
    OwnMutexOnly,
    Unknown,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rights {
    SyntheticWrite,
    ReadOnly,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallStatus {
    Success,
    Failure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ReadbackState {
    Before,
    After,
    Neither,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct StageAssessment {
    pub state: ReadbackState,
    pub confirmed: bool,
}

pub(super) struct ReadbackBinding<'a> {
    pub plan: &'a Plan,
    pub protection: &'a ProtectedRecord,
    pub rights: Rights,
    pub exclusion: Exclusion,
    pub epoch: u64,
}

pub(super) fn assess_stage_readback(
    binding: ReadbackBinding<'_>,
    stage: Stage,
    snapshot: Result<&Inventory, Reason>,
    status: CallStatus,
) -> StageAssessment {
    let unknown = StageAssessment {
        state: ReadbackState::Neither,
        confirmed: false,
    };
    if validate_access(binding.rights, binding.exclusion, binding.epoch).is_err() {
        return unknown;
    }
    let (Some(index), Ok(snapshot)) = (MUTATION_STAGES.iter().position(|s| *s == stage), snapshot)
    else {
        return unknown;
    };
    let before = validate_snapshot(binding.plan, binding.protection, index, snapshot).is_ok();
    let after = validate_snapshot(binding.plan, binding.protection, index + 1, snapshot).is_ok();
    let state = match (before, after) {
        (true, false) => ReadbackState::Before,
        (false, true) => ReadbackState::After,
        _ => ReadbackState::Neither,
    };
    StageAssessment {
        state,
        confirmed: status == CallStatus::Success && state == ReadbackState::After,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadFault {
    Denied,
    InvalidType,
    Generation,
    Different,
    Reordered,
    Duplicate,
    ProtectedChanged,
    ClassChanged,
    TargetStackUnknown,
    OsStackUnknown,
    BeforeState,
    AfterState,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GuardFault {
    Generation,
    Removed,
    SwapWithOs,
    ExclusionLost,
    Loaded,
    RightsLost,
    ProtectedRecordChanged,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Fault {
    IntentSave(Stage),
    CrashBefore(Stage),
    CrashAfter(Stage),
    ApiError(Stage, bool),
    Readback(Stage, ReadFault),
    ConfirmSave(Stage),
    Guard(Stage, GuardFault),
    Verify(ReadFault),
    CrashDuringV,
    CrashBeforeComplete,
    CompleteSave,
    CallObservation {
        stage: Stage,
        status: CallStatus,
        effect: bool,
        readback: ReadFault,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Classification {
    Rejected,
    ConfirmedPrefix,
    Unknown,
    RegistrationReadbackMatch,
    AlreadyConfirmed,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Outcome {
    pub classification: Classification,
    pub reason: Option<Reason>,
    pub confirmed: Vec<Stage>,
    pub observed_effects: Vec<Stage>,
    pub driver_installed_or_loaded: bool,
    pub registered_or_attached: bool,
    pub observed_live_requests: bool,
    pub real_device_access: bool,
    pub runtime_qualified: bool,
    pub windows_client_qualified: bool,
}
impl Outcome {
    fn new(classification: Classification, reason: Option<Reason>, confirmed: Vec<Stage>) -> Self {
        Self {
            classification,
            reason,
            confirmed,
            observed_effects: vec![],
            driver_installed_or_loaded: false,
            registered_or_attached: false,
            observed_live_requests: false,
            real_device_access: false,
            runtime_qualified: false,
            windows_client_qualified: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Checkpoint {
    IntentSaved(Stage),
    EffectApplied(Stage),
    VerificationStarted,
    VerificationObserved,
    BeforeCompletion,
}

pub struct SyntheticLab {
    pub inventory: Inventory,
    pub protection: ProtectedRecord,
    pub exclusion: Exclusion,
    pub rights: Rights,
    pub journal_writable: bool,
    pub journal: Option<Journal>,
    /// Independent recorder: all attempted mutation targets, including failed calls.
    pub calls: Vec<Mutation>,
    /// Independent store recorder: changes that actually occurred, not journal ACKs.
    pub changes: Vec<Mutation>,
    /// Independent full-snapshot reads returned to the algorithm, including V.
    pub reads: Vec<Stage>,
    pub fault: Option<Fault>,
    started: bool,
}

impl SyntheticLab {
    pub fn new(inventory: Inventory, protection: ProtectedRecord) -> Self {
        Self {
            inventory,
            protection,
            exclusion: Exclusion::WholeLab { epoch: 1 },
            rights: Rights::SyntheticWrite,
            journal_writable: true,
            journal: None,
            calls: vec![],
            changes: vec![],
            reads: vec![],
            fault: None,
            started: false,
        }
    }

    /// A restarted fake has no fresh-attempt capability, even if its record was lost.
    pub fn restart(
        inventory: Inventory,
        protection: ProtectedRecord,
        journal: Option<Journal>,
    ) -> Self {
        let mut lab = Self::new(inventory, protection);
        lab.journal = journal;
        lab.started = true;
        lab
    }

    pub fn apply(&mut self, plan: &Plan) -> Outcome {
        self.apply_inner(plan, |_, _| {})
    }

    /// Observer is for isolated fake process cutpoint tests; it never supplies effects.
    #[cfg(test)]
    pub(super) fn apply_observed(
        &mut self,
        plan: &Plan,
        checkpoint: impl FnMut(&Self, Checkpoint),
    ) -> Outcome {
        self.apply_inner(plan, checkpoint)
    }

    fn apply_inner(
        &mut self,
        plan: &Plan,
        mut checkpoint: impl FnMut(&Self, Checkpoint),
    ) -> Outcome {
        if self.started || self.journal.is_some() {
            return self.restart_read_only(plan);
        }
        let epoch = match self.exclusion {
            Exclusion::WholeLab { epoch } => epoch,
            _ => {
                return Outcome::new(
                    Classification::Rejected,
                    Some(Reason::ExclusionUnavailable),
                    vec![],
                );
            }
        };
        if self.rights != Rights::SyntheticWrite {
            return Outcome::new(
                Classification::Rejected,
                Some(Reason::RightsUnavailable),
                vec![],
            );
        }
        if !self.journal_writable {
            return Outcome::new(
                Classification::Rejected,
                Some(Reason::JournalUnavailable),
                vec![],
            );
        }
        if let Err(reason) = self.guard(plan, 0, epoch) {
            return Outcome::new(Classification::Rejected, Some(reason), vec![]);
        }
        self.journal = Some(Journal {
            bound: plan.clone(),
            events: vec![],
        });
        self.started = true;
        let mut confirmed = vec![];
        for (index, stage) in MUTATION_STAGES.into_iter().enumerate() {
            if let Some(Fault::Guard(s, fault)) = self.fault
                && s == stage
            {
                self.inject_guard(plan, fault);
            }
            if let Err(reason) = self.guard(plan, index, epoch) {
                return stopped(confirmed, reason);
            }
            if self.fault == Some(Fault::IntentSave(stage)) {
                return stopped(confirmed, Reason::JournalUnavailable);
            }
            self.journal
                .as_mut()
                .unwrap()
                .events
                .push(JournalEvent::Intent(stage));
            checkpoint(self, Checkpoint::IntentSaved(stage));
            if self.fault == Some(Fault::CrashBefore(stage)) {
                return Outcome::new(
                    Classification::Unknown,
                    Some(Reason::EffectUnknown),
                    confirmed,
                );
            }
            let status = self.call_store(request(plan, stage));
            checkpoint(self, Checkpoint::EffectApplied(stage));
            if self.fault == Some(Fault::CrashAfter(stage)) {
                return Outcome::new(
                    Classification::Unknown,
                    Some(Reason::EffectUnknown),
                    confirmed,
                );
            }
            self.journal
                .as_mut()
                .unwrap()
                .events
                .push(JournalEvent::Returned(stage, status == CallStatus::Success));
            let observed = self.readback(plan, stage);
            let assessment = assess_stage_readback(
                ReadbackBinding {
                    plan,
                    protection: &self.protection,
                    rights: self.rights,
                    exclusion: self.exclusion,
                    epoch,
                },
                stage,
                observed.as_ref().map_err(|reason| *reason),
                status,
            );
            if !assessment.confirmed {
                let mut result = Outcome::new(
                    Classification::Unknown,
                    Some(Reason::EffectUnknown),
                    confirmed,
                );
                if assessment.state == ReadbackState::After {
                    result.observed_effects.push(stage);
                }
                return result;
            }
            if self.fault == Some(Fault::ConfirmSave(stage)) {
                return Outcome::new(
                    Classification::Unknown,
                    Some(Reason::JournalUnavailable),
                    confirmed,
                );
            }
            self.journal
                .as_mut()
                .unwrap()
                .events
                .push(JournalEvent::Confirmed(stage));
            confirmed.push(stage);
        }
        checkpoint(self, Checkpoint::VerificationStarted);
        if self.fault == Some(Fault::CrashDuringV) {
            return Outcome::new(
                Classification::Unknown,
                Some(Reason::IncompleteAttempt),
                confirmed,
            );
        }
        if let Some(Fault::Verify(fault)) = self.fault {
            self.inject_read_fault(plan, fault);
        }
        let observed = self.readback(plan, Stage::V);
        checkpoint(self, Checkpoint::VerificationObserved);
        if validate_access(self.rights, self.exclusion, epoch).is_err()
            || !observed
                .as_ref()
                .is_ok_and(|snapshot| self.validate_snapshot(plan, 4, snapshot).is_ok())
        {
            return Outcome::new(
                Classification::Unknown,
                Some(Reason::StateChanged),
                confirmed,
            );
        }
        checkpoint(self, Checkpoint::BeforeCompletion);
        if matches!(
            self.fault,
            Some(Fault::CrashBeforeComplete | Fault::CompleteSave)
        ) {
            return Outcome::new(
                Classification::Unknown,
                Some(Reason::JournalUnavailable),
                confirmed,
            );
        }
        self.journal
            .as_mut()
            .unwrap()
            .events
            .push(JournalEvent::Complete);
        Outcome::new(Classification::RegistrationReadbackMatch, None, confirmed)
    }

    /// Always uses current enumeration; never writes or resumes an incomplete attempt.
    pub fn restart_read_only(&self, plan: &Plan) -> Outcome {
        let Some(journal) = &self.journal else {
            return Outcome::new(
                Classification::Unknown,
                Some(Reason::IncompleteAttempt),
                vec![],
            );
        };
        let confirmed: Vec<_> = journal
            .events
            .iter()
            .filter_map(|event| {
                if let JournalEvent::Confirmed(s) = event {
                    Some(*s)
                } else {
                    None
                }
            })
            .collect();
        let exact_events: Vec<_> = MUTATION_STAGES
            .into_iter()
            .flat_map(|s| {
                [
                    JournalEvent::Intent(s),
                    JournalEvent::Returned(s, true),
                    JournalEvent::Confirmed(s),
                ]
            })
            .chain([JournalEvent::Complete])
            .collect();
        if journal.bound == *plan
            && journal.events == exact_events
            && self.validate_state(plan, 4).is_ok()
        {
            return Outcome::new(Classification::AlreadyConfirmed, None, confirmed);
        }
        let mut result = Outcome::new(
            Classification::Unknown,
            Some(Reason::IncompleteAttempt),
            confirmed,
        );
        if journal.bound != *plan {
            result.reason = Some(Reason::StateChanged);
            return result;
        }
        if self.protection != plan.protection {
            result.reason = Some(Reason::ProtectedDisk);
            return result;
        }
        // The fresh full snapshot can identify an exact prefix, but cannot confer completion/ownership.
        for count in 0..=4 {
            let expected = expected(plan, count);
            if self.inventory == expected {
                result.observed_effects = MUTATION_STAGES[..count].to_vec();
                break;
            }
        }
        result
    }

    fn guard(&self, plan: &Plan, applied: usize, epoch: u64) -> Result<(), Reason> {
        validate_access(self.rights, self.exclusion, epoch)?;
        self.validate_state(plan, applied)
    }

    fn validate_state(&self, plan: &Plan, applied: usize) -> Result<(), Reason> {
        self.validate_snapshot(plan, applied, &self.inventory)
    }

    pub(super) fn execute_store(&mut self, request: StoreRequest, apply: bool) {
        let (stage, target) = match &request {
            StoreRequest::Package(_) => (Stage::P, Target::Package),
            StoreRequest::Service(_) => (Stage::S, Target::Service),
            StoreRequest::Selector(_) => (Stage::K, Target::Selector),
            StoreRequest::Upper { object, .. } => (Stage::F, Target::Devnode(*object)),
        };
        let mutation = Mutation {
            stage,
            target,
            actor: Actor::Registrar,
        };
        self.calls.push(mutation.clone());
        if !apply {
            return;
        }
        match request {
            StoreRequest::Package(package) => {
                self.inventory.package = Observed::Known(Some(package))
            }
            StoreRequest::Service(service) => self.inventory.service = service,
            StoreRequest::Selector(value) => self.inventory.selector = value,
            StoreRequest::Upper {
                object,
                generation,
                value,
            } => {
                let Some(disk) = self.inventory.disks.iter_mut().find(|disk| {
                    disk.object == object && disk.generation == generation && disk.present
                }) else {
                    return;
                };
                disk.upper = value;
            }
        }
        self.changes.push(mutation);
    }

    // Only the fake store knows whether a failed call changed its private state.
    fn call_store(&mut self, request: StoreRequest) -> CallStatus {
        let stage = match &request {
            StoreRequest::Package(_) => Stage::P,
            StoreRequest::Service(_) => Stage::S,
            StoreRequest::Selector(_) => Stage::K,
            StoreRequest::Upper { .. } => Stage::F,
        };
        let (apply, status) = match self.fault {
            Some(Fault::ApiError(s, effect)) if s == stage => (effect, CallStatus::Failure),
            Some(Fault::CallObservation {
                stage: s,
                status,
                effect,
                ..
            }) if s == stage => (effect, status),
            _ => (true, CallStatus::Success),
        };
        self.execute_store(request, apply);
        status
    }

    fn validate_snapshot(
        &self,
        plan: &Plan,
        applied: usize,
        snapshot: &Inventory,
    ) -> Result<(), Reason> {
        validate_snapshot(plan, &self.protection, applied, snapshot)
    }

    fn readback(&mut self, plan: &Plan, stage: Stage) -> Result<Inventory, Reason> {
        self.reads.push(stage);
        let fault = match self.fault {
            Some(Fault::Readback(s, fault)) if s == stage => Some(fault),
            Some(Fault::CallObservation {
                stage: s, readback, ..
            }) if s == stage => Some(readback),
            Some(Fault::Verify(fault @ (ReadFault::Denied | ReadFault::InvalidType)))
                if stage == Stage::V =>
            {
                Some(fault)
            }
            _ => None,
        };
        if fault == Some(ReadFault::Denied) {
            return Err(Reason::PropertyUnavailable);
        }
        let mut snapshot = self.inventory.clone();
        match fault {
            Some(ReadFault::BeforeState) => {
                snapshot = expected(
                    plan,
                    MUTATION_STAGES
                        .iter()
                        .position(|s| *s == stage)
                        .unwrap_or(4),
                )
            }
            Some(ReadFault::AfterState) => {
                snapshot = expected(
                    plan,
                    MUTATION_STAGES
                        .iter()
                        .position(|s| *s == stage)
                        .map_or(4, |n| n + 1),
                )
            }
            Some(ReadFault::Generation) => {
                snapshot
                    .disks
                    .iter_mut()
                    .find(|d| d.object == plan.target_object)
                    .unwrap()
                    .generation += 1
            }
            Some(ReadFault::InvalidType) => match stage {
                Stage::P => snapshot.package = Observed::Unknown,
                Stage::S => snapshot.service = ServiceState::Unknown,
                Stage::K => {
                    snapshot.selector = RegistryRead::Value(RawRegistryValue {
                        value_type: 7,
                        bytes: vec![0, 0],
                    })
                }
                Stage::F | Stage::V => {
                    snapshot
                        .disks
                        .iter_mut()
                        .find(|d| d.object == plan.target_object)
                        .unwrap()
                        .upper = RegistryRead::Value(RawRegistryValue {
                        value_type: 1,
                        bytes: vec![0, 0],
                    })
                }
            },
            Some(ReadFault::Different) => match stage {
                Stage::P => snapshot.package = Observed::Known(None),
                Stage::S => snapshot.service = ServiceState::Conflict,
                Stage::K => {
                    snapshot.selector = RegistryRead::Value(plan.protection.os.instance.selector())
                }
                Stage::F | Stage::V => {
                    snapshot
                        .disks
                        .iter_mut()
                        .find(|d| d.object == plan.target_object)
                        .unwrap()
                        .upper = FilterValue::Names(vec!["Wrong".into()]).registry()
                }
            },
            _ => {}
        }
        Ok(snapshot)
    }

    fn inject_guard(&mut self, plan: &Plan, fault: GuardFault) {
        match fault {
            GuardFault::Generation => {
                self.inventory
                    .disks
                    .iter_mut()
                    .find(|d| d.object == plan.target_object)
                    .unwrap()
                    .generation += 1
            }
            GuardFault::Removed => {
                self.inventory
                    .disks
                    .iter_mut()
                    .find(|d| d.object == plan.target_object)
                    .unwrap()
                    .present = false
            }
            GuardFault::SwapWithOs => {
                let target = self
                    .inventory
                    .disks
                    .iter_mut()
                    .find(|d| d.object == plan.target_object)
                    .unwrap();
                target.identity = plan.protection.os.clone();
            }
            GuardFault::ExclusionLost => self.exclusion = Exclusion::Unknown,
            GuardFault::Loaded => self.inventory.service = ServiceState::Loaded,
            GuardFault::RightsLost => self.rights = Rights::ReadOnly,
            GuardFault::ProtectedRecordChanged => self.protection.os.serial.push_str("-changed"),
        }
    }
    fn inject_read_fault(&mut self, plan: &Plan, fault: ReadFault) {
        let foreign_target = match fault {
            ReadFault::Different | ReadFault::Reordered | ReadFault::Duplicate => {
                Some(Target::Devnode(plan.target_object))
            }
            ReadFault::ProtectedChanged => Some(Target::Devnode(
                self.inventory
                    .disks
                    .iter()
                    .find(|d| d.identity.matches(&plan.protection.os))
                    .unwrap()
                    .object,
            )),
            ReadFault::ClassChanged => Some(Target::Class),
            _ => None,
        };
        if let Some(target) = foreign_target {
            let mutation = Mutation {
                stage: Stage::V,
                target,
                actor: Actor::Competitor,
            };
            self.calls.push(mutation.clone());
            self.changes.push(mutation);
        }
        match fault {
            ReadFault::Generation => self.inject_guard(plan, GuardFault::Generation),
            ReadFault::Different => {
                self.inventory
                    .disks
                    .iter_mut()
                    .find(|d| d.object == plan.target_object)
                    .unwrap()
                    .upper = FilterValue::Names(vec!["Wrong".into()]).registry()
            }
            ReadFault::Reordered => {
                self.inventory
                    .disks
                    .iter_mut()
                    .find(|d| d.object == plan.target_object)
                    .unwrap()
                    .upper =
                    FilterValue::Names(vec!["keepB".into(), "KeepA".into(), "Probe".into()])
                        .registry()
            }
            ReadFault::Duplicate => {
                self.inventory
                    .disks
                    .iter_mut()
                    .find(|d| d.object == plan.target_object)
                    .unwrap()
                    .upper = FilterValue::Names(vec![
                    "KeepA".into(),
                    "keepB".into(),
                    "Probe".into(),
                    "Probe".into(),
                ])
                .registry()
            }
            ReadFault::InvalidType => {}
            ReadFault::ProtectedChanged => {
                self.inventory
                    .disks
                    .iter_mut()
                    .find(|d| d.identity.matches(&plan.protection.os))
                    .unwrap()
                    .upper = FilterValue::Names(vec!["Unexpected".into()]).registry()
            }
            ReadFault::ClassChanged => {
                self.inventory.class_upper =
                    FilterValue::Names(vec!["Unexpected".into()]).registry()
            }
            ReadFault::TargetStackUnknown => {
                self.inventory
                    .disks
                    .iter_mut()
                    .find(|d| d.object == plan.target_object)
                    .unwrap()
                    .stack = Observed::Unknown
            }
            ReadFault::OsStackUnknown => {
                self.inventory
                    .disks
                    .iter_mut()
                    .find(|d| d.identity.matches(&plan.protection.os))
                    .unwrap()
                    .stack = Observed::Unknown
            }
            ReadFault::Denied | ReadFault::BeforeState | ReadFault::AfterState => {}
        }
    }
}

fn validate_snapshot(
    plan: &Plan,
    protection: &ProtectedRecord,
    applied: usize,
    snapshot: &Inventory,
) -> Result<(), Reason> {
    if *protection != plan.protection {
        return Err(Reason::ProtectedDisk);
    }
    identify(snapshot, &plan.protection)?;
    let target = plan.target(snapshot).ok_or(Reason::StateChanged)?;
    if !target.present || *snapshot != expected(plan, applied) {
        return Err(Reason::StateChanged);
    }
    Ok(())
}

fn validate_access(rights: Rights, exclusion: Exclusion, epoch: u64) -> Result<(), Reason> {
    if rights != Rights::SyntheticWrite {
        return Err(Reason::RightsUnavailable);
    }
    if exclusion != (Exclusion::WholeLab { epoch }) {
        return Err(Reason::ExclusionUnavailable);
    }
    Ok(())
}

fn stopped(confirmed: Vec<Stage>, reason: Reason) -> Outcome {
    let classification = if confirmed.is_empty() {
        Classification::Rejected
    } else {
        Classification::ConfirmedPrefix
    };
    Outcome::new(classification, Some(reason), confirmed)
}
fn request(plan: &Plan, stage: Stage) -> StoreRequest {
    match stage {
        Stage::P => StoreRequest::Package(plan.package.clone()),
        Stage::S => StoreRequest::Service(ServiceState::DemandStopped {
            name: plan.package.service.clone(),
            image_hash: plan
                .package
                .artifacts
                .iter()
                .find(|a| a.name.ends_with(".sys"))
                .unwrap()
                .sha256,
            owner_attempt: plan.attempt,
        }),
        Stage::K => StoreRequest::Selector(RegistryRead::Value(
            plan.protection.target.instance.selector(),
        )),
        Stage::F => StoreRequest::Upper {
            object: plan.target_object,
            generation: plan.target_generation,
            value: plan.after_upper.clone(),
        },
        Stage::V => unreachable!(),
    }
}
fn expected(plan: &Plan, count: usize) -> Inventory {
    let mut inventory = plan.before.clone();
    for stage in MUTATION_STAGES[..count].iter().copied() {
        mutate(&mut inventory, plan, stage);
    }
    inventory
}
fn mutate(inventory: &mut Inventory, plan: &Plan, stage: Stage) {
    match stage {
        Stage::P => inventory.package = Observed::Known(Some(plan.package.clone())),
        Stage::S => {
            inventory.service = ServiceState::DemandStopped {
                name: plan.package.service.clone(),
                image_hash: plan
                    .package
                    .artifacts
                    .iter()
                    .find(|a| a.name.ends_with(".sys"))
                    .unwrap()
                    .sha256,
                owner_attempt: plan.attempt,
            }
        }
        Stage::K => {
            inventory.selector = RegistryRead::Value(plan.protection.target.instance.selector())
        }
        Stage::F => {
            inventory
                .disks
                .iter_mut()
                .find(|d| d.object == plan.target_object)
                .unwrap()
                .upper = plan.after_upper.clone()
        }
        Stage::V => unreachable!(),
    }
}
