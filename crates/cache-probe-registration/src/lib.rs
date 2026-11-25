#![forbid(unsafe_code)]

//! Read-only registration planning and an in-memory-only synthetic executor.

use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Reason {
    InvalidId,
    InvalidProperty,
    PropertyUnavailable,
    ContextMismatch,
    IdentityMismatch,
    CapacityMismatch,
    ProtectedDisk,
    UsageUnknown,
    SpecialUse,
    StackUnknown,
    PackageRejected,
    ServiceConflict,
    ExistingRegistration,
    ExclusionUnavailable,
    RightsUnavailable,
    StateChanged,
    JournalUnavailable,
    EffectUnknown,
    IncompleteAttempt,
    RealApplyUnavailable,
}

#[derive(Clone, Eq, PartialEq)]
pub struct InstanceId(String);

impl fmt::Debug for InstanceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("InstanceId(<private>)")
    }
}

impl InstanceId {
    pub fn new(value: &str) -> Result<Self, Reason> {
        if value.is_empty() || value.contains('\0') || value.encode_utf16().count() >= 512 {
            return Err(Reason::InvalidId);
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn from_selector(value: &RawRegistryValue) -> Result<Self, Reason> {
        if value.value_type != 1 {
            return Err(Reason::InvalidProperty);
        }
        let units = decode_units(&value.bytes)?;
        if units.last() != Some(&0) || units.len() > 512 {
            return Err(Reason::InvalidProperty);
        }
        let text =
            String::from_utf16(&units[..units.len() - 1]).map_err(|_| Reason::InvalidProperty)?;
        Self::new(&text)
    }

    pub fn selector(&self) -> RawRegistryValue {
        RawRegistryValue {
            value_type: 1,
            bytes: units_bytes(self.0.encode_utf16().chain([0])),
        }
    }

    fn admissible(&self) -> bool {
        !self.0.is_empty()
            && self.0.len() < 200
            && self
                .0
                .bytes()
                .all(|byte| byte.is_ascii_graphic() && byte != b',')
    }

    /// Only admitted PnP ASCII identifiers compare, exactly ignoring ASCII case.
    /// The selector codec can preserve values outside this eligibility domain.
    pub fn matches(&self, other: &Self) -> bool {
        self.admissible() && other.admissible() && self.0.eq_ignore_ascii_case(&other.0)
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct RawRegistryValue {
    pub value_type: u32,
    pub bytes: Vec<u8>,
}
impl fmt::Debug for RawRegistryValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawRegistryValue")
            .field("value_type", &self.value_type)
            .field("bytes", &"<private>")
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegistryRead {
    Missing,
    Value(RawRegistryValue),
    Unavailable,
}

#[derive(Clone, Eq, PartialEq)]
pub enum FilterValue {
    Missing,
    Names(Vec<String>),
}
impl fmt::Debug for FilterValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => f.write_str("Missing"),
            Self::Names(names) => f.debug_tuple("Names").field(&names.len()).finish(),
        }
    }
}

fn decode_units(bytes: &[u8]) -> Result<Vec<u16>, Reason> {
    let (pairs, remainder) = bytes.as_chunks::<2>();
    if !remainder.is_empty() {
        return Err(Reason::InvalidProperty);
    }
    Ok(pairs.iter().copied().map(u16::from_le_bytes).collect())
}

fn units_bytes(units: impl IntoIterator<Item = u16>) -> Vec<u8> {
    units.into_iter().flat_map(u16::to_le_bytes).collect()
}

impl FilterValue {
    pub fn parse(read: &RegistryRead) -> Result<Self, Reason> {
        let value = match read {
            RegistryRead::Missing => return Ok(Self::Missing),
            RegistryRead::Unavailable => return Err(Reason::PropertyUnavailable),
            RegistryRead::Value(value) => value,
        };
        if value.value_type != 7 {
            return Err(Reason::InvalidProperty);
        }
        let units = decode_units(&value.bytes)?;
        if units.is_empty() || units.last() != Some(&0) {
            return Err(Reason::InvalidProperty);
        }
        if units == [0] || units == [0, 0] {
            return Ok(Self::Names(vec![]));
        }
        let mut names = vec![];
        let mut start = 0;
        for (i, unit) in units.iter().enumerate() {
            if *unit != 0 {
                continue;
            }
            if i == start {
                if i != units.len() - 1 {
                    return Err(Reason::InvalidProperty);
                }
                return Ok(Self::Names(names));
            }
            let name = String::from_utf16(&units[start..i]).map_err(|_| Reason::InvalidProperty)?;
            names.push(name);
            start = i + 1;
        }
        Err(Reason::InvalidProperty)
    }

    pub fn registry(&self) -> RegistryRead {
        match self {
            Self::Missing => RegistryRead::Missing,
            Self::Names(names) => RegistryRead::Value(RawRegistryValue {
                value_type: 7,
                bytes: units_bytes(
                    names
                        .iter()
                        .flat_map(|s| s.encode_utf16().chain([0]))
                        .chain([0]),
                ),
            }),
        }
    }

    fn appended(&self, service: &str) -> Result<Self, Reason> {
        let mut names = match self {
            Self::Missing => vec![],
            Self::Names(names) => names.clone(),
        };
        if names.iter().any(|name| name.eq_ignore_ascii_case(service)) {
            return Err(Reason::ExistingRegistration);
        }
        names.push(service.to_owned());
        Ok(Self::Names(names))
    }
}

/// Private evidence records deliberately have no value-bearing Debug formatter.
#[derive(Clone, Eq, PartialEq)]
pub struct Identity {
    pub instance: InstanceId,
    pub storage_id: String,
    pub serial: String,
    pub parent_location: String,
    pub lun: u32,
    pub host_chain: Vec<String>,
}
impl fmt::Debug for Identity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Identity(<private>)")
    }
}
impl Identity {
    fn valid(&self) -> bool {
        [&self.storage_id, &self.serial, &self.parent_location]
            .iter()
            .all(|s| !s.is_empty() && !s.contains('\0'))
            && !self.host_chain.is_empty()
            && self
                .host_chain
                .iter()
                .all(|s| !s.is_empty() && !s.contains('\0'))
    }
    fn matches(&self, other: &Self) -> bool {
        self.instance.matches(&other.instance)
            && self.storage_id == other.storage_id
            && self.serial == other.serial
            && self.parent_location == other.parent_location
            && self.lun == other.lun
            && self.host_chain == other.host_chain
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpecialUse {
    Efi,
    Os,
    Pagefile,
    Dump,
    DedicatedDump,
    Hibernation,
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UseLocation {
    pub usage: SpecialUse,
    pub partition: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Observed<T> {
    Known(T),
    Unknown,
}
impl<T> Observed<T> {
    fn get(&self) -> Result<&T, Reason> {
        match self {
            Self::Known(v) => Ok(v),
            Self::Unknown => Err(Reason::PropertyUnavailable),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Disk {
    /// Synthetic opaque object identity, never a Windows handle or disk number.
    pub object: u64,
    pub generation: u64,
    pub number: u32,
    pub capacity_bytes: Observed<u64>,
    pub hardware_ids: Vec<String>,
    pub compatible_ids: Vec<String>,
    pub location_paths: Observed<Vec<String>>,
    pub label: String,
    pub identity: Identity,
    pub present: bool,
    pub raw: bool,
    pub partitions: u32,
    pub uses: Observed<Vec<UseLocation>>,
    pub class: Observed<String>,
    pub stack: Observed<Vec<String>>,
    pub upper: RegistryRead,
    pub lower: RegistryRead,
    pub function_service: String,
}

#[derive(Clone, Eq, PartialEq)]
pub struct Context {
    pub vm: String,
    pub build: String,
    pub snapshot: String,
}
impl fmt::Debug for Context {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Context(<private>)")
    }
}
impl Context {
    fn valid(&self) -> bool {
        [&self.vm, &self.build, &self.snapshot]
            .iter()
            .all(|s| !s.is_empty() && !s.contains('\0'))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Artifact {
    pub name: String,
    pub sha256: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PackageEffect {
    StageOnly,
    DemandKernelService { name: String, binary: String },
    CopyFile { name: String },
    Forbidden(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageManifest {
    pub service: String,
    pub version: String,
    pub artifacts: Vec<Artifact>,
    /// Expanded effects of every applicable section and referenced dependency.
    pub sections: Vec<PackageEffect>,
}

impl PackageManifest {
    fn validate(&self, reviewed: &Self) -> Result<(), Reason> {
        if self != reviewed
            || self.service.is_empty()
            || !self
                .service
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
            || self.version.is_empty()
            || self.artifacts.len() != 3
        {
            return Err(Reason::PackageRejected);
        }
        let mut extensions = vec![];
        for artifact in &self.artifacts {
            if artifact.name.contains(['/', '\\', ':', '\0']) || artifact.sha256 == [0; 32] {
                return Err(Reason::PackageRejected);
            }
            extensions.push(
                artifact
                    .name
                    .rsplit('.')
                    .next()
                    .unwrap_or("")
                    .to_ascii_lowercase(),
            );
        }
        extensions.sort();
        if extensions != ["cat", "inf", "sys"] {
            return Err(Reason::PackageRejected);
        }
        let mut stage = 0;
        let mut service = 0;
        for effect in &self.sections {
            match effect {
                PackageEffect::StageOnly => stage += 1,
                PackageEffect::DemandKernelService { name, binary }
                    if name == &self.service
                        && self
                            .artifacts
                            .iter()
                            .any(|a| a.name == *binary && a.name.ends_with(".sys")) =>
                {
                    service += 1
                }
                PackageEffect::CopyFile { name }
                    if self.artifacts.iter().any(|a| a.name == *name) => {}
                _ => return Err(Reason::PackageRejected),
            }
        }
        if stage != 1 || service != 1 {
            return Err(Reason::PackageRejected);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServiceState {
    Absent,
    DemandStopped {
        name: String,
        image_hash: [u8; 32],
        owner_attempt: u64,
    },
    Conflict,
    Loaded,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Inventory {
    pub context: Observed<Context>,
    pub disks: Vec<Disk>,
    pub class_upper: RegistryRead,
    pub class_lower: RegistryRead,
    pub package: Observed<Option<PackageManifest>>,
    pub service: ServiceState,
    pub selector: RegistryRead,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtectedRecord {
    pub context: Context,
    pub os: Identity,
    pub target: Identity,
    pub os_uses: Vec<UseLocation>,
    pub expected_class: String,
    pub expected_stack: Vec<String>,
}

#[derive(Clone, Eq, PartialEq)]
pub struct Plan {
    attempt: u64,
    before: Inventory,
    protection: ProtectedRecord,
    package: PackageManifest,
    target_object: u64,
    target_generation: u64,
    after_upper: RegistryRead,
    os_usage_changes: Vec<UseLocation>,
}
impl fmt::Debug for Plan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Plan")
            .field("attempt", &self.attempt)
            .field("target", &"<private>")
            .field("effects", &["P", "S", "K", "F", "V"])
            .finish()
    }
}

impl Plan {
    pub fn attempt(&self) -> u64 {
        self.attempt
    }
    pub fn target_object(&self) -> u64 {
        self.target_object
    }
    pub fn target_generation(&self) -> u64 {
        self.target_generation
    }
    pub fn original_upper(&self) -> &RegistryRead {
        &self.target(&self.before).expect("bound target").upper
    }
    pub fn expected_upper(&self) -> &RegistryRead {
        &self.after_upper
    }
    pub fn package(&self) -> &PackageManifest {
        &self.package
    }
    pub fn target_instance(&self) -> &InstanceId {
        &self.protection.target.instance
    }
    pub fn os_usage_changes(&self) -> &[UseLocation] {
        &self.os_usage_changes
    }
    /// No token, fixture assertion or administrator privilege opens a real apply path.
    pub fn real_apply_gate(&self) -> Result<(), Reason> {
        Err(Reason::RealApplyUnavailable)
    }
    fn target<'a>(&self, inventory: &'a Inventory) -> Option<&'a Disk> {
        inventory
            .disks
            .iter()
            .find(|d| d.object == self.target_object && d.generation == self.target_generation)
    }
}

fn select<'a>(inventory: &'a Inventory, id: &Identity) -> Result<&'a Disk, Reason> {
    if !id.valid() {
        return Err(Reason::IdentityMismatch);
    }
    let candidates: Vec<_> = inventory
        .disks
        .iter()
        .filter(|disk| disk.identity.matches(id))
        .collect();
    if candidates.len() != 1 || !candidates[0].present {
        return Err(Reason::IdentityMismatch);
    }
    let disk = candidates[0];
    // A duplicate of any independently bound key is ambiguous, even if the other keys differ.
    if inventory.disks.iter().any(|other| {
        other.object != disk.object
            && (other.identity.instance.matches(&disk.identity.instance)
                || other.identity.storage_id == disk.identity.storage_id
                || other.identity.serial == disk.identity.serial)
    }) {
        return Err(Reason::IdentityMismatch);
    }
    Ok(disk)
}

fn identify<'a>(
    inventory: &'a Inventory,
    protection: &ProtectedRecord,
) -> Result<(&'a Disk, &'a Disk), Reason> {
    if !protection.context.valid() || inventory.context.get()? != &protection.context {
        return Err(Reason::ContextMismatch);
    }
    // Every observed ID participates in collision checks; an inadmissible alias
    // cannot safely be declared distinct by an ASCII-only comparison.
    if !protection.os.instance.admissible()
        || !protection.target.instance.admissible()
        || inventory
            .disks
            .iter()
            .any(|disk| !disk.identity.instance.admissible())
    {
        return Err(Reason::InvalidId);
    }
    if !known_name(&protection.expected_class)
        || protection.expected_stack.is_empty()
        || protection
            .expected_stack
            .iter()
            .any(|name| !known_name(name))
    {
        return Err(Reason::StackUnknown);
    }
    let os = select(inventory, &protection.os)?;
    let target = select(inventory, &protection.target)?;
    if os.object == target.object || os.identity.instance.matches(&target.identity.instance) {
        return Err(Reason::ProtectedDisk);
    }
    if *target.capacity_bytes.get()? != 8 << 30 {
        return Err(Reason::CapacityMismatch);
    }
    let os_uses = os.uses.get().map_err(|_| Reason::UsageUnknown)?;
    if !os_uses.iter().any(|u| u.usage == SpecialUse::Os)
        || !os_uses.iter().any(|u| u.usage == SpecialUse::Efi)
    {
        return Err(Reason::ProtectedDisk);
    }
    if !target
        .uses
        .get()
        .map_err(|_| Reason::UsageUnknown)?
        .is_empty()
    {
        return Err(Reason::SpecialUse);
    }
    if !target.raw || target.partitions != 0 {
        return Err(Reason::SpecialUse);
    }
    for disk in [os, target] {
        if !known_name(&disk.function_service)
            || !protection
                .expected_stack
                .iter()
                .any(|name| name.eq_ignore_ascii_case(&disk.function_service))
            || disk.class.get().map_err(|_| Reason::StackUnknown)? != &protection.expected_class
            || disk.stack.get().map_err(|_| Reason::StackUnknown)? != &protection.expected_stack
        {
            return Err(Reason::StackUnknown);
        }
    }
    // Every baseline must be readable; unknown must never become an empty value.
    for (index, disk) in inventory.disks.iter().enumerate() {
        disk.uses.get().map_err(|_| Reason::UsageUnknown)?;
        FilterValue::parse(&disk.upper)?;
        FilterValue::parse(&disk.lower)?;
        if disk.object == 0
            || inventory.disks[..index]
                .iter()
                .any(|d| d.object == disk.object)
        {
            return Err(Reason::IdentityMismatch);
        }
    }
    FilterValue::parse(&inventory.class_upper)?;
    FilterValue::parse(&inventory.class_lower)?;
    Ok((os, target))
}

fn known_name(value: &str) -> bool {
    !value.trim().is_empty() && !value.chars().any(char::is_control)
}

pub fn make_plan(
    attempt: u64,
    observed: &Inventory,
    protection: &ProtectedRecord,
    package: &PackageManifest,
    reviewed: &PackageManifest,
) -> Result<Plan, Reason> {
    if attempt == 0 {
        return Err(Reason::IncompleteAttempt);
    }
    package.validate(reviewed)?;
    let (os, target) = identify(observed, protection)?;
    if observed.service != ServiceState::Absent
        || observed.package.get()?.is_some()
        || observed.selector != RegistryRead::Missing
    {
        return Err(Reason::ServiceConflict);
    }
    let after_upper = FilterValue::parse(&target.upper)?
        .appended(&package.service)?
        .registry();
    let changes = if os.uses.get()? == &protection.os_uses {
        vec![]
    } else {
        os.uses.get()?.clone()
    };
    Ok(Plan {
        attempt,
        before: observed.clone(),
        protection: protection.clone(),
        package: package.clone(),
        target_object: target.object,
        target_generation: target.generation,
        after_upper,
        os_usage_changes: changes,
    })
}

pub mod synthetic;

#[cfg(test)]
mod tests;
