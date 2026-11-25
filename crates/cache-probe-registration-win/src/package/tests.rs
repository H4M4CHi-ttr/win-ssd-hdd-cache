use super::*;

#[test]
fn b04_proposal_all_sections_and_references_are_allowlisted() {
    inspect_inf(PROPOSAL).unwrap();
    let sections = parse(PROPOSAL).unwrap();
    assert_eq!(sections.len(), 8);
    assert_eq!(
        sections["defaultinstall.ntamd64.services"],
        vec!["AddService=WinSsdHddCacheProbeP04,0,P04.Service"]
    );
    assert!(sections["p04.service"].contains(&"StartType=3".to_owned()));
}

#[test]
fn r07_b04_mutated_broad_or_automatic_effects_fail_closed() {
    // Each negative fixture mutates actual proposal bytes, not a policy boolean.
    let mutations = [
        PROPOSAL.replace("StartType=3", "StartType=0"),
        PROPOSAL.replace("StartType=3", "StartType=1"),
        PROPOSAL.replace("StartType=3", "StartType=2"),
        PROPOSAL.replace("P04,0,P04.Service", "P04,2,P04.Service"),
        PROPOSAL.replace("P04,0,P04.Service", "P04,0x800,P04.Service"),
        PROPOSAL.replace("Class=System", "Class=Extension"),
        PROPOSAL.replace("CopyFiles=P04.Copy", "CopyFiles=P04.Copy,Other.Copy"),
        PROPOSAL.replace(
            "ServiceBinary=%13%\\p04-observer.sys",
            "ServiceBinary=%12%\\other.sys",
        ),
        format!(
            "{PROPOSAL}\n[Manufacturer]\nP04=Models,NTamd64\n[Models.NTamd64]\nP04=Install,GENERIC_DISK_ID\n"
        ),
        format!("{PROPOSAL}\n[DefaultInstall.NTx86]\nCopyFiles=Other.Copy\n"),
        format!(
            "{PROPOSAL}\n[DefaultInstall.NTamd64.10.0]\nInclude=other.inf\nNeeds=Other.Install\n"
        ),
        PROPOSAL.replace(
            "CopyFiles=P04.Copy",
            "CopyFiles=P04.Copy\nAddReg=P04.ClassFilters",
        ),
        format!("{PROPOSAL}\n[ClassInstall32]\nAddReg=GlobalFilters\n"),
        PROPOSAL.replace("ErrorControl=1", "ErrorControl=1\nAddTrigger=P04.Trigger"),
        PROPOSAL.replace(
            "CopyFiles=P04.Copy",
            "CopyFiles=P04.Copy\nCoInstallers32=other.dll",
        ),
    ];
    for (index, mutated) in mutations.iter().enumerate() {
        assert_ne!(mutated, PROPOSAL, "fixture {index} must alter bytes");
        assert!(inspect_inf(mutated).is_err(), "forbidden fixture {index}");
    }
}

#[test]
fn b04_duplicate_sections_unknown_directives_and_missing_references_fail() {
    for inf in [
        format!("{PROPOSAL}\n[Version]\n"),
        PROPOSAL.replace("[P04.Copy]", "[Other.Copy]"),
        PROPOSAL.replace("StartType=3", "StartType=3\nStartType=0"),
        PROPOSAL.replace("PnpLockdown=1", "Include=surprise.inf"),
        "StartType=3".to_owned(),
        PROPOSAL.replace("System", "System\0"),
    ] {
        assert!(inspect_inf(&inf).is_err());
    }
}

fn files() -> [PackageFile<'static>; 3] {
    [
        PackageFile {
            name: "p04-observer.inf",
            bytes: PROPOSAL.as_bytes(),
        },
        PackageFile {
            name: "p04-observer.cat",
            bytes: b"SYNTHETIC NOT SIGNED CAT",
        },
        PackageFile {
            name: "p04-observer.sys",
            bytes: b"SYNTHETIC NOT A DRIVER",
        },
    ]
}

#[test]
fn b04_actual_bytes_bind_manifest_but_never_grant_deployment() {
    let files = files();
    let proof = inspect_package(&files).unwrap();
    assert_eq!(proof.manifest().service, "WinSsdHddCacheProbeP04");
    assert_eq!(proof.manifest().artifacts.len(), 3);
    assert_eq!(
        proof.deployment_review(),
        Err(PackageError::MissingSignedCatalog)
    );
    let mut changed = files;
    changed[2].bytes = b"DIFFERENT SYNTHETIC BYTES";
    assert_ne!(
        proof.manifest().artifacts[2].sha256,
        inspect_package(&changed).unwrap().manifest().artifacts[2].sha256
    );
}

#[test]
fn b04_missing_cat_or_sys_and_extra_inf_are_rejected() {
    let initial = files();
    assert!(inspect_package(&initial[..1]).is_err());
    assert!(inspect_package(&initial[..2]).is_err());
    let mut extra = Vec::from(initial);
    extra.push(PackageFile {
        name: "extension.inf",
        bytes: b"unreviewed",
    });
    assert!(matches!(
        inspect_package(&extra),
        Err(PackageError::ExtraArtifact)
    ));
    let mut duplicate = files();
    duplicate[1].name = "p04-observer.sys";
    assert!(inspect_package(&duplicate).is_err());
}

#[test]
fn b04_sha256_known_independent_vectors() {
    fn hex(bytes: [u8; 32]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }
    assert_eq!(
        hex(sha256(b"")),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        hex(sha256(b"abc")),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        hex(sha256(
            b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
        )),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
}

#[test]
fn b04_inspected_package_binds_core_plan_and_hash_mutation_is_rejected() {
    use cache_probe_registration::*;
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
            parent_location: "SYNTHETIC-controller".into(),
            lun,
            host_chain: vec![
                format!("SYNTHETIC-{name}-r1"),
                format!("SYNTHETIC-{name}-base"),
            ],
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
    let disks = [(10, os.clone()), (20, target.clone())]
        .into_iter()
        .map(|(object, identity)| Disk {
            object,
            generation: 4,
            number: object as u32,
            identity,
            present: true,
            raw: object == 20,
            partitions: if object == 10 { 3 } else { 0 },
            uses: Observed::Known(if object == 10 { uses.clone() } else { vec![] }),
            class: Observed::Known("DiskDrive".into()),
            stack: Observed::Known(stack.clone()),
            upper: RegistryRead::Missing,
            lower: RegistryRead::Missing,
            function_service: "disk".into(),
            capacity_bytes: Observed::Known(8 * 1024 * 1024 * 1024),
            hardware_ids: vec!["SYNTHETIC-SHARED-ID".into()],
            compatible_ids: vec!["SYNTHETIC-SHARED-COMPAT".into()],
            location_paths: Observed::Known(vec![]),
            label: "SYNTHETIC-LABEL".into(),
        })
        .collect();
    let observed = Inventory {
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
    let files = files();
    let inspected = inspect_package(&files).unwrap();
    let reviewed = inspected.manifest().clone();
    let plan = make_plan(91, &observed, &protection, inspected.manifest(), &reviewed).unwrap();
    assert_eq!(plan.target_object(), 20);
    assert_eq!(plan.real_apply_gate(), Err(Reason::RealApplyUnavailable));
    assert_eq!(
        inspected.deployment_review(),
        Err(PackageError::MissingSignedCatalog)
    );
    let mut altered = reviewed.clone();
    altered.artifacts[2].sha256[0] ^= 1;
    assert_eq!(
        make_plan(91, &observed, &protection, &altered, &reviewed),
        Err(Reason::PackageRejected)
    );
}
