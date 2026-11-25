use super::super::tests::Fake;
use super::*;
use cache_probe_registration::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Fault {
    None,
    Source,
    Preexisting,
    StageError,
    StageReadback,
    StageAutoService,
    ServiceError,
    BootStart,
    Loaded,
    WrongImage,
    WrongHash,
}
struct Port {
    raw: Fake,
    package: PackageManifest,
    staged: bool,
    service: ServiceView,
    effects: Vec<Stage>,
    image_reads: Vec<PathBuf>,
    fault: Fault,
}
impl Leaves for Port {
    fn create_set(&mut self) -> Result<usize, Error> {
        self.raw.create_set()
    }
    fn destroy_set(&mut self, set: usize) {
        self.raw.destroy_set(set)
    }
    fn open_exact(&mut self, set: usize, id: &[u16], data: &mut DevInfo) -> Result<(), Error> {
        self.raw.open_exact(set, id, data)
    }
    fn instance(&mut self, set: usize, data: &mut DevInfo) -> Result<Vec<u16>, Error> {
        self.raw.instance(set, data)
    }
    fn equal_id(&mut self, a: &[u16], b: &[u16]) -> Result<bool, Error> {
        self.raw.equal_id(a, b)
    }
    fn filters(&mut self, set: usize, data: &mut DevInfo) -> Result<Property, Error> {
        self.raw.filters(set, data)
    }
    fn set_filters(&mut self, set: usize, data: &mut DevInfo, bytes: &[u8]) -> Result<(), Error> {
        self.raw.set_filters(set, data, bytes)
    }
    fn open_selector_key(&mut self) -> Result<usize, Error> {
        self.raw.open_selector_key()
    }
    fn close_key(&mut self, key: usize) {
        self.raw.close_key(key)
    }
    fn selector(&mut self, key: usize) -> Result<Property, Error> {
        self.raw.selector(key)
    }
    fn set_selector(&mut self, key: usize, bytes: &[u8]) -> Result<(), Error> {
        self.raw.set_selector(key, bytes)
    }
}
fn image() -> PathBuf {
    PathBuf::from("SYNTHETIC/p04-observer.sys")
}
impl Port {
    fn ready(&self) -> ServiceView {
        ServiceView::Present {
            kind: 1,
            start: 3,
            error: 1,
            state: 1,
            image: image().to_string_lossy().encode_utf16().collect(),
        }
    }
}
impl StageLeaves for Port {
    fn approved_image_hash(&mut self, receipt: &Receipt) -> Result<[u8; 32], Error> {
        self.image_reads.push(receipt.image.clone());
        let mut hash = self.package.artifacts[2].sha256;
        if self.fault == Fault::WrongHash {
            hash[0] ^= 1;
        }
        Ok(hash)
    }
    fn source_matches(&mut self, _: &Source, package: &PackageManifest) -> Result<(), Error> {
        if self.fault == Fault::Source || package != &self.package {
            Err(Error::Changed)
        } else {
            Ok(())
        }
    }
    fn service_view(&mut self) -> Result<ServiceView, Error> {
        if self.fault == Fault::Preexisting && self.effects.is_empty() {
            return Ok(self.ready());
        }
        Ok(self.service.clone())
    }
    fn copy_inf(&mut self, _: &Source, flags: u32) -> Result<Receipt, Error> {
        assert_eq!(flags, 8);
        self.effects.push(Stage::P);
        self.staged = true;
        if self.fault == Fault::StageError {
            return Err(Error::WriteUnknown);
        }
        if self.fault == Fault::StageAutoService {
            self.service = self.ready();
        }
        Ok(Receipt {
            published: "SYNTHETIC/oem42.inf".into(),
            store_inf: "SYNTHETIC/p04-observer.inf".into(),
            image: image(),
        })
    }
    fn staged_matches(&mut self, _: &Receipt, package: &PackageManifest) -> Result<(), Error> {
        assert!(self.staged);
        assert_eq!(package, &self.package);
        if self.fault == Fault::StageReadback {
            Err(Error::Readback)
        } else {
            Ok(())
        }
    }
    fn install_service_section(
        &mut self,
        _: &Receipt,
        section: &str,
        flags: u32,
    ) -> Result<(), Error> {
        assert_eq!(section, "DefaultInstall.NTamd64.Services");
        assert_eq!(flags, 0);
        self.effects.push(Stage::S);
        self.service = self.ready();
        if let ServiceView::Present {
            start,
            state,
            image,
            ..
        } = &mut self.service
        {
            match self.fault {
                Fault::BootStart => *start = 0,
                Fault::Loaded => *state = 4,
                Fault::WrongImage => *image = "SYNTHETIC/other.sys".encode_utf16().collect(),
                _ => {}
            }
        }
        if self.fault == Fault::ServiceError {
            Err(Error::WriteUnknown)
        } else {
            Ok(())
        }
    }
}

fn fixture(fault: Fault) -> (Plan, Port, Source) {
    let files = [
        crate::package::PackageFile {
            name: "p04-observer.inf",
            bytes: crate::package::PROPOSAL.as_bytes(),
        },
        crate::package::PackageFile {
            name: "p04-observer.cat",
            bytes: b"SYNTHETIC CAT UNSIGNED",
        },
        crate::package::PackageFile {
            name: "p04-observer.sys",
            bytes: b"SYNTHETIC SYS NOT DRIVER",
        },
    ];
    let proof = crate::package::inspect_package(&files).unwrap();
    let package = proof.manifest().clone();
    let context = Context {
        vm: "SYNTHETIC-VM".into(),
        build: "SYNTHETIC-BUILD".into(),
        snapshot: "R1".into(),
    };
    fn identity(name: &str, lun: u32) -> Identity {
        Identity {
            instance: InstanceId::new(&format!("SYNTHETIC\\{name}")).unwrap(),
            storage_id: format!("storage-{name}"),
            serial: format!("serial-{name}"),
            parent_location: "SYNTHETIC-CONTROLLER".into(),
            lun,
            host_chain: vec![format!("SYNTHETIC-{name}-R1")],
        }
    }
    let os = identity("OS", 0);
    let target = identity("T1", 1);
    let uses = vec![
        UseLocation {
            usage: SpecialUse::Efi,
            partition: 1,
        },
        UseLocation {
            usage: SpecialUse::Os,
            partition: 3,
        },
        UseLocation {
            usage: SpecialUse::Pagefile,
            partition: 3,
        },
        UseLocation {
            usage: SpecialUse::Dump,
            partition: 3,
        },
    ];
    let stack = vec![
        "partmgr".to_owned(),
        "disk".to_owned(),
        "stornvme".to_owned(),
    ];
    let disks = [(11, os.clone()), (22, target.clone())]
        .into_iter()
        .map(|(object, identity)| Disk {
            object,
            generation: 7,
            number: object as u32,
            identity,
            present: true,
            raw: object == 22,
            partitions: if object == 11 { 3 } else { 0 },
            uses: Observed::Known(if object == 11 { uses.clone() } else { vec![] }),
            class: Observed::Known("DiskDrive".into()),
            stack: Observed::Known(stack.clone()),
            upper: RegistryRead::Missing,
            lower: RegistryRead::Missing,
            function_service: "disk".into(),
            capacity_bytes: Observed::Known(8 * 1024 * 1024 * 1024),
            hardware_ids: vec!["SYNTHETIC-SHARED".into()],
            compatible_ids: vec![],
            location_paths: Observed::Known(vec![]),
            label: "SYNTHETIC".into(),
        })
        .collect();
    let inventory = Inventory {
        context: Observed::Known(context.clone()),
        disks,
        class_upper: RegistryRead::Missing,
        class_lower: RegistryRead::Missing,
        package: Observed::Known(None),
        service: ServiceState::Absent,
        selector: RegistryRead::Missing,
    };
    let protection = ProtectedRecord {
        context,
        os,
        target,
        os_uses: uses,
        expected_class: "DiskDrive".into(),
        expected_stack: stack,
    };
    let plan = make_plan(701, &inventory, &protection, &package, &package).unwrap();
    let port = Port {
        raw: Fake::new(),
        package,
        staged: false,
        service: ServiceView::Absent,
        effects: vec![],
        image_reads: vec![],
        fault,
    };
    (
        plan,
        port,
        Source {
            directory: PathBuf::from("SYNTHETIC"),
        },
    )
}

#[test]
fn b02_same_plan_native_port_p_s_k_f_and_no_real_capability() {
    let (plan, mut port, source) = fixture(Fault::None);
    assert_eq!(
        registration_prefix(&mut port, &plan, &source),
        Ok([Stage::P, Stage::S, Stage::K, Stage::F])
    );
    assert_eq!(port.effects, [Stage::P, Stage::S]);
    assert_eq!(
        port.raw
            .calls
            .iter()
            .filter(|&&call| call == "selector-set")
            .count(),
        1
    );
    assert_eq!(port.raw.effects.len(), 1);
    assert_eq!(port.raw.effects[0].0, 17);
    assert_eq!(plan.real_apply_gate(), Err(Reason::RealApplyUnavailable));
    assert_eq!(
        String::from_utf16(&port.raw.id[..port.raw.id.len() - 1]).unwrap(),
        plan.target_instance().as_str()
    );
}

#[test]
fn r07_unreviewed_source_or_existing_service_has_zero_effects() {
    for fault in [Fault::Source, Fault::Preexisting] {
        let (plan, mut port, source) = fixture(fault);
        assert!(registration_prefix(&mut port, &plan, &source).is_err());
        assert!(port.effects.is_empty());
        assert!(port.raw.effects.is_empty());
    }
}

#[test]
fn r13_p_error_or_readback_or_automatic_service_stops_before_s() {
    for fault in [
        Fault::StageError,
        Fault::StageReadback,
        Fault::StageAutoService,
    ] {
        let (plan, mut port, source) = fixture(fault);
        assert!(registration_prefix(&mut port, &plan, &source).is_err());
        assert_eq!(port.effects, [Stage::P]);
        assert!(port.raw.effects.is_empty());
        assert!(!port.raw.calls.contains(&"selector-set"));
        assert!(port.staged);
    }
}

#[test]
fn r07_s_start_loaded_wrong_binary_or_api_error_never_reaches_k_f() {
    for fault in [
        Fault::ServiceError,
        Fault::BootStart,
        Fault::Loaded,
        Fault::WrongImage,
        Fault::WrongHash,
    ] {
        let (plan, mut port, source) = fixture(fault);
        assert!(registration_prefix(&mut port, &plan, &source).is_err());
        assert_eq!(port.effects, [Stage::P, Stage::S]);
        assert!(port.raw.effects.is_empty());
        assert!(!port.raw.calls.contains(&"selector-set"));
    }
}

#[test]
fn b02_service_config_abi_and_path_outputs_are_bounded() {
    assert!(returned_path(vec![65, 0], 261).is_err());
    assert!(returned_path(vec![65, 66], 2).is_err());
    assert!(wide_path(Path::new("relative.inf")).is_err());
    let mut receipt = Receipt {
        published: "SYNTHETIC/other.inf".into(),
        store_inf: "SYNTHETIC/p04-observer.inf".into(),
        image: image(),
    };
    assert_eq!(receipt.validate(), Err(Error::Readback));
    receipt.published = "SYNTHETIC/oem42.inf".into();
    receipt.image = "OTHER/p04-observer.sys".into();
    assert_eq!(receipt.validate(), Err(Error::Readback));
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        assert_eq!(std::mem::size_of::<super::native::ServiceConfig>(), 64);
        assert_eq!(
            std::mem::offset_of!(super::native::ServiceConfig, binary),
            16
        );
    }
}

#[test]
fn r07_conflicting_service_location_never_opens_or_hashes_image() {
    for location in [
        r"C:\other\unreviewed.sys",
        r"\\server\share\secret.sys",
        r"\\.\PhysicalDrive0",
        r"\??\C:\private\secret",
        r"C:\pkg\probe.sys:secret",
        r"C:\pkg\CON",
    ] {
        let (plan, mut port, _) = fixture(Fault::None);
        let receipt = Receipt {
            published: "SYNTHETIC/oem42.inf".into(),
            store_inf: "SYNTHETIC/p04-observer.inf".into(),
            image: image(),
        };
        port.service = port.ready();
        if let ServiceView::Present { image, .. } = &mut port.service {
            *image = location.encode_utf16().collect();
        }
        assert_eq!(
            verified_service(&mut port, &receipt, plan.package()),
            Err(Error::Readback)
        );
        assert!(
            port.image_reads.is_empty(),
            "conflicting SCM metadata must not trigger a file open/hash"
        );
        assert!(port.effects.is_empty());
    }
    let (plan, mut port, source) = fixture(Fault::Preexisting);
    assert!(registration_prefix(&mut port, &plan, &source).is_err());
    assert!(port.image_reads.is_empty());
    assert!(port.effects.is_empty());
}

struct FilePort {
    metadata_calls: Vec<PathBuf>,
    reads: Vec<PathBuf>,
    reparse_at: Option<usize>,
    denied_at: Option<usize>,
    nonregular: bool,
}
impl LocalFiles for FilePort {
    fn metadata(&mut self, path: &Path) -> Result<FileMetadata, Error> {
        let index = self.metadata_calls.len();
        self.metadata_calls.push(path.to_path_buf());
        if self.denied_at == Some(index) {
            return Err(Error::Query);
        }
        Ok(FileMetadata {
            regular: index == 2 && !self.nonregular,
            directory: index < 2,
            reparse: self.reparse_at == Some(index),
        })
    }
    fn read_regular(&mut self, path: &Path, maximum: u64) -> Result<Vec<u8>, Error> {
        assert_eq!(maximum, 64 * 1024 * 1024);
        self.reads.push(path.to_path_buf());
        Ok(b"SYNTHETIC approved bytes".to_vec())
    }
}
fn file_port() -> FilePort {
    FilePort {
        metadata_calls: vec![],
        reads: vec![],
        reparse_at: None,
        denied_at: None,
        nonregular: false,
    }
}

#[test]
fn r07_namespace_reserved_reparse_and_nonregular_paths_never_read() {
    for path in [
        r"\\server\share\probe.sys",
        r"\\?\C:\pkg\probe.sys",
        r"\\.\PhysicalDrive0",
        r"\??\C:\pkg\probe.sys",
        r"\Device\HarddiskVolume1\probe.sys",
        r"C:\pkg\probe.sys:secret",
        r"C:\pkg\CON",
        r"C:\pkg\COM1.txt",
        r"C:\pkg\LPT²",
        r"C:\pkg\NUL.sys",
        r"C:\pkg\CON .txt",
        r"C:\pkg\..\probe.sys",
        r"C:\pkg\trailing. ",
        r"C:\pkg\\probe.sys",
        r"C:relative.sys",
        "C:\\pkg\\nul\0.sys",
    ] {
        let mut files = file_port();
        assert!(checked_read(&mut files, Path::new(path)).is_err());
        assert!(files.metadata_calls.is_empty());
        assert!(files.reads.is_empty());
    }
    for index in 0..3 {
        let mut files = file_port();
        files.reparse_at = Some(index);
        assert_eq!(
            checked_read(&mut files, Path::new(r"C:\pkg\probe.sys")),
            Err(Error::Query)
        );
        assert!(files.reads.is_empty());
        let mut denied = file_port();
        denied.denied_at = Some(index);
        assert_eq!(
            checked_read(&mut denied, Path::new(r"C:\pkg\probe.sys")),
            Err(Error::Query)
        );
        assert!(denied.reads.is_empty());
    }
    let mut not_regular = file_port();
    not_regular.nonregular = true;
    assert_eq!(
        checked_read(&mut not_regular, Path::new(r"C:\pkg\probe.sys")),
        Err(Error::Query)
    );
    assert!(not_regular.reads.is_empty());
    let mut approved = file_port();
    assert_eq!(
        checked_read(&mut approved, Path::new(r"C:\pkg\probe.sys")).unwrap(),
        b"SYNTHETIC approved bytes"
    );
    assert_eq!(approved.metadata_calls.len(), 3);
    assert_eq!(approved.reads, vec![PathBuf::from(r"C:\pkg\probe.sys")]);
}
