use super::*;
use synthetic::*;

mod crash_tests;

const A: u64 = 11;
const T: u64 = 22;
const U: u64 = 33;

fn identity(name: &str, lun: u32) -> Identity {
    Identity {
        instance: InstanceId::new(&format!("SYNTHETIC\\DISK\\{name}")).unwrap(),
        storage_id: format!("storage-{name}"),
        serial: format!("serial-{name}"),
        parent_location: "same-controller".into(),
        lun,
        host_chain: vec![format!("private-{name}-r1"), format!("private-{name}-base")],
    }
}
fn uses() -> Vec<UseLocation> {
    vec![
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
    ]
}
fn fixture() -> (Inventory, ProtectedRecord, PackageManifest) {
    let context = Context {
        vm: "anonymous-vm".into(),
        build: "reviewed-build".into(),
        snapshot: "R1".into(),
    };
    let mut disks = vec![];
    for (object, name, lun) in [(A, "A", 0), (T, "T", 1), (U, "U", 2)] {
        disks.push(Disk {
            object,
            generation: 7,
            number: object as u32,
            identity: identity(name, lun),
            present: true,
            capacity_bytes: Observed::Known(if object == A { 64 << 30 } else { 8 << 30 }),
            hardware_ids: vec!["NVME\\SAME_MODEL".into()],
            compatible_ids: vec!["GENERIC_DISK".into()],
            location_paths: Observed::Known(vec![]),
            label: name.into(),
            raw: object != A,
            partitions: if object == A { 3 } else { 0 },
            uses: Observed::Known(if object == A { uses() } else { vec![] }),
            class: Observed::Known("DiskDrive".into()),
            stack: Observed::Known(vec!["partmgr".into(), "disk".into(), "stornvme".into()]),
            upper: FilterValue::Names(if object == T {
                vec!["KeepA".into(), "keepB".into()]
            } else {
                vec!["Retain".into()]
            })
            .registry(),
            lower: FilterValue::Names(vec!["UnchangedLower".into()]).registry(),
            function_service: "disk".into(),
        });
    }
    let protection = ProtectedRecord {
        context: context.clone(),
        os: identity("A", 0),
        target: identity("T", 1),
        os_uses: uses(),
        expected_class: "DiskDrive".into(),
        expected_stack: vec!["partmgr".into(), "disk".into(), "stornvme".into()],
    };
    let package = PackageManifest {
        service: "Probe".into(),
        version: "0.1".into(),
        artifacts: vec![
            Artifact {
                name: "probe.inf".into(),
                sha256: [1; 32],
            },
            Artifact {
                name: "probe.cat".into(),
                sha256: [2; 32],
            },
            Artifact {
                name: "probe.sys".into(),
                sha256: [3; 32],
            },
        ],
        sections: vec![
            PackageEffect::StageOnly,
            PackageEffect::DemandKernelService {
                name: "Probe".into(),
                binary: "probe.sys".into(),
            },
            PackageEffect::CopyFile {
                name: "probe.sys".into(),
            },
        ],
    };
    (
        Inventory {
            context: Observed::Known(context),
            disks,
            class_upper: FilterValue::Names(vec!["ClassKeep".into()]).registry(),
            class_lower: RegistryRead::Missing,
            package: Observed::Known(None),
            service: ServiceState::Absent,
            selector: RegistryRead::Missing,
        },
        protection,
        package,
    )
}
fn plan() -> (Plan, SyntheticLab) {
    let (inventory, protection, package) = fixture();
    let plan = make_plan(42, &inventory, &protection, &package, &package).unwrap();
    (plan, SyntheticLab::new(inventory, protection))
}
fn target(inventory: &mut Inventory) -> &mut Disk {
    inventory.disks.iter_mut().find(|d| d.object == T).unwrap()
}

/// Hard-coded oracle: neither targets nor expected values come from engine output.
fn oracle(lab: &SyntheticLab, count: usize) {
    let expected_targets = [
        Target::Package,
        Target::Service,
        Target::Selector,
        Target::Devnode(T),
    ];
    let expected_stages = [Stage::P, Stage::S, Stage::K, Stage::F];
    assert_eq!(lab.calls.len(), count);
    for (index, call) in lab.calls.iter().enumerate() {
        assert_eq!(call.stage, expected_stages[index]);
        assert_eq!(call.target, expected_targets[index]);
        assert_eq!(call.actor, Actor::Registrar);
    }
    assert!(lab.calls.iter().all(|m| !matches!(
        m.target,
        Target::Devnode(A | U)
            | Target::Class
            | Target::Lower(_)
            | Target::Function(_)
            | Target::Vm
    )));
    let baseline = fixture().0;
    for object in [A, U] {
        assert_eq!(
            lab.inventory.disks.iter().find(|d| d.object == object),
            baseline.disks.iter().find(|d| d.object == object)
        );
    }
    assert_eq!(lab.inventory.class_upper, baseline.class_upper);
    assert_eq!(lab.inventory.class_lower, baseline.class_lower);
    let disk = lab.inventory.disks.iter().find(|d| d.object == T).unwrap();
    assert_eq!(disk.lower, baseline.disks[1].lower);
    assert_eq!(disk.function_service, "disk");
}
fn assert_rejected(inventory: Inventory, protection: ProtectedRecord, package: PackageManifest) {
    // A read-only plan cannot emit a call even when input parsing fails.
    let original = inventory.clone();
    assert!(make_plan(42, &inventory, &protection, &package, &package).is_err());
    assert_eq!(inventory, original);
    let lab = SyntheticLab::new(inventory, protection);
    assert!(lab.calls.is_empty());
    assert!(lab.changes.is_empty());
}

#[test]
fn r01_dry_run_then_p_s_k_f_v_only_t() {
    let (plan, mut lab) = plan();
    oracle(&lab, 0);
    let outcome = lab.apply(&plan);
    assert_eq!(
        outcome.classification,
        Classification::RegistrationReadbackMatch
    );
    oracle(&lab, 4);
    assert_eq!(lab.changes.len(), 4);
    assert_eq!(
        lab.reads,
        [Stage::P, Stage::S, Stage::K, Stage::F, Stage::V]
    );
    assert_eq!(
        FilterValue::parse(&lab.inventory.disks[1].upper),
        Ok(FilterValue::Names(vec![
            "KeepA".into(),
            "keepB".into(),
            "Probe".into()
        ]))
    );
    assert_eq!(
        InstanceId::from_selector(match &lab.inventory.selector {
            RegistryRead::Value(v) => v,
            _ => panic!("selector absent"),
        })
        .unwrap(),
        identity("T", 1).instance
    );
    assert!(
        !outcome.driver_installed_or_loaded
            && !outcome.registered_or_attached
            && !outcome.real_device_access
            && !outcome.runtime_qualified
            && !outcome.windows_client_qualified
    );
    assert_eq!(plan.real_apply_gate(), Err(Reason::RealApplyUnavailable));
}

#[test]
fn r02_input_points_to_os() {
    let (inventory, mut protection, package) = fixture();
    protection.target = protection.os.clone();
    assert_rejected(inventory, protection, package);
}
#[test]
fn r02_t_label_has_a_identity() {
    let (mut inventory, protection, package) = fixture();
    target(&mut inventory).identity = protection.os.clone();
    assert_rejected(inventory, protection, package);
}
#[test]
fn r02_a_reuses_t_number() {
    let (mut inventory, protection, package) = fixture();
    inventory.disks[0].number = 22;
    inventory.disks[0].raw = true;
    inventory.disks[0].partitions = 0;
    inventory.disks[0].capacity_bytes = Observed::Known(8 << 30);
    inventory.disks[0].label = "T".into();
    let plan = make_plan(42, &inventory, &protection, &package, &package).unwrap();
    assert_eq!(plan.target_object(), T);
}
#[test]
fn r02_api_returns_a_after_t_check() {
    let (plan, mut lab) = plan();
    lab.fault = Some(Fault::Guard(Stage::F, GuardFault::SwapWithOs));
    let result = lab.apply(&plan);
    assert_ne!(
        result.classification,
        Classification::RegistrationReadbackMatch
    );
    assert_eq!(
        lab.calls.iter().map(|m| m.stage).collect::<Vec<_>>(),
        [Stage::P, Stage::S, Stage::K]
    );
    assert!(lab.calls.iter().all(|m| m.target != Target::Devnode(A)));
}

macro_rules! rejection {
    ($name:ident, $change:expr) => {
        #[test]
        fn $name() {
            let (mut i, mut p, mut k) = fixture();
            ($change)(&mut i, &mut p, &mut k);
            assert_rejected(i, p, k);
        }
    };
}
rejection!(
    r03_zero_t,
    |i: &mut Inventory, _: &mut ProtectedRecord, _: &mut PackageManifest| i
        .disks
        .retain(|d| d.object != T)
);
rejection!(r03_two_t, |i: &mut Inventory,
                       _: &mut ProtectedRecord,
                       _: &mut PackageManifest| {
    let mut duplicate = i.disks[1].clone();
    duplicate.object = 44;
    i.disks.push(duplicate);
});
rejection!(
    r03_disconnected,
    |i: &mut Inventory, _: &mut ProtectedRecord, _: &mut PackageManifest| target(i).present = false
);
rejection!(
    r03_duplicate_serial,
    |i: &mut Inventory, _: &mut ProtectedRecord, _: &mut PackageManifest| i.disks[2]
        .identity
        .serial =
        i.disks[1].identity.serial.clone()
);
rejection!(
    r03_mixed_identity_rows,
    |i: &mut Inventory, _: &mut ProtectedRecord, _: &mut PackageManifest| {
        i.disks[2].identity.storage_id = i.disks[1].identity.storage_id.clone();
        i.disks[1].identity.storage_id = "wrong".into();
    }
);
rejection!(
    r03_old_host_chain,
    |i: &mut Inventory, _: &mut ProtectedRecord, _: &mut PackageManifest| target(i)
        .identity
        .host_chain =
        vec!["old-T1".into()]
);
#[test]
fn r03_number_changes_parent_lun_identity_still_bind() {
    let (mut i, p, k) = fixture();
    target(&mut i).number = 99;
    let plan = make_plan(42, &i, &p, &k, &k).unwrap();
    let mut lab = SyntheticLab::new(i, p);
    assert_eq!(
        lab.apply(&plan).classification,
        Classification::RegistrationReadbackMatch
    );
    oracle(&lab, 4);
}

#[test]
fn r04_selector_malformed_and_exact_matching() {
    for value in [
        RawRegistryValue {
            value_type: 7,
            bytes: vec![0, 0],
        },
        RawRegistryValue {
            value_type: 1,
            bytes: vec![65],
        },
        RawRegistryValue {
            value_type: 1,
            bytes: vec![65, 0],
        },
        RawRegistryValue {
            value_type: 1,
            bytes: vec![65, 0, 0, 0, 66, 0, 0, 0],
        },
        RawRegistryValue {
            value_type: 1,
            bytes: vec![0, 0],
        },
    ] {
        assert!(InstanceId::from_selector(&value).is_err());
    }
    assert!(
        InstanceId::new("SYNTHETIC\\DISK\\t")
            .unwrap()
            .matches(&identity("T", 1).instance)
    );
    assert!(
        !InstanceId::new("SYNTHETIC\\DISK")
            .unwrap()
            .matches(&identity("T", 1).instance)
    );
    assert!(
        !InstanceId::new(" SYNTHETIC\\DISK\\T")
            .unwrap()
            .matches(&identity("T", 1).instance)
    );
}

macro_rules! special_use {
    ($name:ident,$usage:ident) => {
        rejection!($name, |i: &mut Inventory,
                           _: &mut ProtectedRecord,
                           _: &mut PackageManifest| target(
            i
        )
        .uses =
            Observed::Known(vec![UseLocation {
                usage: SpecialUse::$usage,
                partition: 1
            }]));
    };
}
special_use!(r05_efi, Efi);
special_use!(r05_os, Os);
special_use!(r05_pagefile, Pagefile);
special_use!(r05_dump, Dump);
special_use!(r05_dedicated_dump, DedicatedDump);
special_use!(r05_hibernation, Hibernation);
special_use!(r05_other, Other);
rejection!(
    r05_unknown_usage,
    |i: &mut Inventory, _: &mut ProtectedRecord, _: &mut PackageManifest| target(i).uses =
        Observed::Unknown
);
rejection!(
    r05_unknown_build,
    |i: &mut Inventory, _: &mut ProtectedRecord, _: &mut PackageManifest| i.context =
        Observed::Unknown
);
rejection!(
    r05_unknown_vm,
    |i: &mut Inventory, _: &mut ProtectedRecord, _: &mut PackageManifest| if let Observed::Known(
        c,
    ) = &mut i.context
    {
        c.vm.clear();
    }
);
rejection!(
    r05_unknown_r1,
    |i: &mut Inventory, _: &mut ProtectedRecord, _: &mut PackageManifest| if let Observed::Known(
        c,
    ) = &mut i.context
    {
        c.snapshot.clear();
    }
);
rejection!(
    r05_unknown_chain,
    |i: &mut Inventory, _: &mut ProtectedRecord, _: &mut PackageManifest| target(i)
        .identity
        .host_chain
        .clear()
);
#[test]
fn r05_os_pagefile_change_is_retained_not_target_use() {
    let (mut i, p, k) = fixture();
    if let Observed::Known(uses) = &mut i.disks[0].uses {
        uses.iter_mut()
            .find(|u| u.usage == SpecialUse::Pagefile)
            .unwrap()
            .partition = 4;
    }
    let plan = make_plan(42, &i, &p, &k, &k).unwrap();
    assert_eq!(
        plan.os_usage_changes()
            .iter()
            .find(|u| u.usage == SpecialUse::Pagefile)
            .unwrap()
            .partition,
        4
    );
}

#[test]
fn r06_missing_empty_and_existing_preserved() {
    for read in [
        RegistryRead::Missing,
        FilterValue::Names(vec![]).registry(),
        FilterValue::Names(vec!["z".into(), "A".into(), "a".into()]).registry(),
    ] {
        let (mut i, p, k) = fixture();
        target(&mut i).upper = read.clone();
        let plan = make_plan(42, &i, &p, &k, &k).unwrap();
        assert_eq!(plan.original_upper(), &read);
        let mut lab = SyntheticLab::new(i, p);
        assert_eq!(
            lab.apply(&plan).classification,
            Classification::RegistrationReadbackMatch
        );
        let expected = match FilterValue::parse(&read).unwrap() {
            FilterValue::Missing => vec!["Probe".to_owned()],
            FilterValue::Names(mut n) => {
                n.push("Probe".into());
                n
            }
        };
        assert_eq!(
            FilterValue::parse(&lab.inventory.disks[1].upper),
            Ok(FilterValue::Names(expected))
        );
        oracle(&lab, 4);
    }
}
#[test]
fn r06_invalid_multi_sz_rejected() {
    for read in [
        RegistryRead::Unavailable,
        RegistryRead::Value(RawRegistryValue {
            value_type: 1,
            bytes: vec![0, 0],
        }),
        RegistryRead::Value(RawRegistryValue {
            value_type: 7,
            bytes: vec![0],
        }),
        RegistryRead::Value(RawRegistryValue {
            value_type: 7,
            bytes: units_bytes([65, 0]),
        }),
        RegistryRead::Value(RawRegistryValue {
            value_type: 7,
            bytes: units_bytes([65, 0, 0, 66, 0, 0]),
        }),
        RegistryRead::Value(RawRegistryValue {
            value_type: 7,
            bytes: vec![],
        }),
    ] {
        let (mut i, p, k) = fixture();
        target(&mut i).upper = read;
        assert_rejected(i, p, k);
    }
}
rejection!(
    r07_existing_probe,
    |i: &mut Inventory, _: &mut ProtectedRecord, _: &mut PackageManifest| target(i).upper =
        FilterValue::Names(vec!["Probe".into()]).registry()
);
rejection!(
    r07_duplicate_case_probe,
    |i: &mut Inventory, _: &mut ProtectedRecord, _: &mut PackageManifest| target(i).upper =
        FilterValue::Names(vec!["pRoBe".into(), "Probe".into()]).registry()
);
rejection!(
    r07_wrong_binary_same_service,
    |i: &mut Inventory, _: &mut ProtectedRecord, _: &mut PackageManifest| i.service =
        ServiceState::DemandStopped {
            name: "Probe".into(),
            image_hash: [9; 32],
            owner_attempt: 1
        }
);
rejection!(
    r07_loaded_service,
    |i: &mut Inventory, _: &mut ProtectedRecord, _: &mut PackageManifest| i.service =
        ServiceState::Loaded
);
rejection!(
    r07_unknown_service,
    |i: &mut Inventory, _: &mut ProtectedRecord, _: &mut PackageManifest| i.service =
        ServiceState::Unknown
);
rejection!(
    r07_broad_install,
    |_: &mut Inventory, _: &mut ProtectedRecord, k: &mut PackageManifest| k.sections.push(
        PackageEffect::Forbidden("InstallAllMatchingHardwareIds".into())
    )
);
rejection!(
    r07_class_install,
    |_: &mut Inventory, _: &mut ProtectedRecord, k: &mut PackageManifest| k
        .sections
        .push(PackageEffect::Forbidden("ClassUpperFilters".into()))
);
rejection!(
    r07_auto_start,
    |_: &mut Inventory, _: &mut ProtectedRecord, k: &mut PackageManifest| k
        .sections
        .push(PackageEffect::Forbidden("StartService".into()))
);
rejection!(
    r07_unreviewed_inf,
    |_: &mut Inventory, _: &mut ProtectedRecord, k: &mut PackageManifest| k.artifacts.push(
        Artifact {
            name: "extra.inf".into(),
            sha256: [8; 32]
        }
    )
);
#[test]
fn r07_allowlist_hash_change_rejected() {
    let (i, p, k) = fixture();
    let mut unreviewed = k.clone();
    unreviewed.artifacts[0].sha256 = [9; 32];
    assert_eq!(
        make_plan(42, &i, &p, &unreviewed, &k),
        Err(Reason::PackageRejected)
    );
}

#[test]
fn r08_mutex_and_unknown_exclusion_fail_before_p() {
    for exclusion in [Exclusion::OwnMutexOnly, Exclusion::Unknown] {
        let (plan, mut lab) = plan();
        lab.exclusion = exclusion;
        assert_eq!(lab.apply(&plan).classification, Classification::Rejected);
        oracle(&lab, 0);
    }
}
#[test]
fn r08_real_gate_cannot_be_unlocked_by_synthetic_rights() {
    let (plan, mut lab) = plan();
    assert_eq!(
        lab.apply(&plan).classification,
        Classification::RegistrationReadbackMatch
    );
    assert_eq!(plan.real_apply_gate(), Err(Reason::RealApplyUnavailable));
}
#[test]
fn r07_mutation_oracle_catches_broad_registration() {
    for wrong in [
        Target::Class,
        Target::Devnode(A),
        Target::Devnode(U),
        Target::Lower(T),
    ] {
        let (plan, mut lab) = plan();
        lab.apply(&plan);
        // Deliberately mutate the successful call trace: final values alone cannot detect this.
        lab.calls[3].target = wrong;
        lab.changes[3].target = wrong;
        assert!(std::panic::catch_unwind(|| oracle(&lab, 4)).is_err());
    }
}
#[test]
fn r02_mutated_registrar_request_is_logged_by_actual_fake_object() {
    for object in [A, U] {
        let (plan, mut lab) = plan();
        lab.fault = Some(Fault::Guard(Stage::F, GuardFault::ExclusionLost));
        lab.apply(&plan);
        lab.execute_store(
            StoreRequest::Upper {
                object,
                generation: 7,
                value: plan.after_upper.clone(),
            },
            true,
        );
        assert_eq!(lab.calls[3].target, Target::Devnode(object));
        assert_eq!(lab.changes[3].target, Target::Devnode(object));
        assert!(std::panic::catch_unwind(|| oracle(&lab, 4)).is_err());
    }
}
#[test]
fn r08_compare_then_set_mutation_loses_other_even_with_matching_readback() {
    let mut competitor_list = vec!["KeepA".to_owned(), "keepB".to_owned(), "Other".to_owned()];
    let old = competitor_list.clone();
    competitor_list = vec!["KeepA".into(), "keepB".into(), "Probe".into()];
    // A readback equal to the registrar's intended list does not preserve Other.
    assert!(!old.iter().all(|n| competitor_list.contains(n)));
}

macro_rules! guard_case {
    ($name:ident,$fault:ident) => {
        #[test]
        fn $name() {
            let (plan, mut lab) = plan();
            lab.fault = Some(Fault::Guard(Stage::F, GuardFault::$fault));
            let result = lab.apply(&plan);
            assert_ne!(
                result.classification,
                Classification::RegistrationReadbackMatch
            );
            assert_eq!(
                lab.calls.iter().map(|m| m.stage).collect::<Vec<_>>(),
                [Stage::P, Stage::S, Stage::K]
            );
            assert_eq!(lab.changes.len(), 3);
            assert!(
                lab.calls
                    .iter()
                    .all(|m| m.target != Target::Devnode(A) && m.target != Target::Devnode(T))
            );
        }
    };
}
guard_case!(r09_generation, Generation);
guard_case!(r09_removed, Removed);
guard_case!(r09_swap, SwapWithOs);
guard_case!(r09_exclusion_lost, ExclusionLost);
guard_case!(r09_loaded, Loaded);
guard_case!(r09_rights_lost, RightsLost);
guard_case!(r09_protected_record_changed, ProtectedRecordChanged);

macro_rules! verify_case {
    ($name:ident,$fault:ident) => {
        #[test]
        fn $name() {
            let (plan, mut lab) = plan();
            lab.fault = Some(Fault::Verify(ReadFault::$fault));
            let result = lab.apply(&plan);
            assert_eq!(result.classification, Classification::Unknown);
            assert_eq!(
                lab.calls
                    .iter()
                    .filter(|m| m.actor == Actor::Registrar)
                    .count(),
                4
            );
            assert_eq!(
                lab.changes
                    .iter()
                    .filter(|m| m.actor == Actor::Registrar)
                    .count(),
                4
            );
            assert!(
                lab.calls
                    .iter()
                    .filter(|m| m.actor == Actor::Registrar)
                    .all(|m| m.target != Target::Devnode(A) && m.target != Target::Class)
            );
            assert!(!result.registered_or_attached && !result.windows_client_qualified);
        }
    };
}
verify_case!(r10_target_content, Different);
verify_case!(r10_target_order, Reordered);
verify_case!(r10_target_duplicate_count, Duplicate);
verify_case!(r10_type, InvalidType);
verify_case!(r10_target_generation, Generation);
verify_case!(r10_protected_changed, ProtectedChanged);
verify_case!(r10_class_changed, ClassChanged);
verify_case!(r10_t_stack_unknown, TargetStackUnknown);
verify_case!(r10_a_stack_unknown, OsStackUnknown);
verify_case!(r10_v_denied, Denied);

#[test]
fn r11_completed_same_attempt_is_read_only() {
    let (plan, mut lab) = plan();
    lab.apply(&plan);
    let calls = lab.calls.clone();
    let changes = lab.changes.clone();
    assert_eq!(
        lab.apply(&plan).classification,
        Classification::AlreadyConfirmed
    );
    assert_eq!(lab.calls, calls);
    assert_eq!(lab.changes, changes);
}
#[test]
fn r11_values_without_completion_never_adopted() {
    let (plan, mut lab) = plan();
    lab.apply(&plan);
    lab.journal = Some(lab.journal.as_ref().unwrap().without_completion());
    let calls = lab.calls.len();
    assert_eq!(lab.apply(&plan).classification, Classification::Unknown);
    assert_eq!(lab.calls.len(), calls);
    lab.journal = None;
    assert_eq!(lab.apply(&plan).classification, Classification::Unknown);
    assert_eq!(lab.calls.len(), calls);
}
#[test]
fn r11_new_generation_number_reuse_is_not_owned() {
    let (plan, mut lab) = plan();
    lab.apply(&plan);
    target(&mut lab.inventory).generation += 1;
    let calls = lab.calls.len();
    assert_eq!(lab.apply(&plan).classification, Classification::Unknown);
    assert_eq!(lab.calls.len(), calls);
}
#[test]
fn r11_different_bound_plan_never_adopted() {
    let (plan, mut lab) = plan();
    let (i, p, k) = fixture();
    let other = make_plan(99, &i, &p, &k, &k).unwrap();
    lab.apply(&plan);
    let calls = lab.calls.len();
    assert_eq!(lab.apply(&other).classification, Classification::Unknown);
    assert_eq!(lab.calls.len(), calls);
}
#[test]
fn r12_public_outcomes_redact_private_markers() {
    let (mut i, mut p, k) = fixture();
    let marker = "SECRET_INSTANCE_SERIAL_C:\\Users\\Secret";
    target(&mut i).identity.serial = marker.into();
    p.target.serial = marker.into();
    let plan = make_plan(42, &i, &p, &k, &k).unwrap();
    let mut lab = SyntheticLab::new(i, p);
    lab.fault = Some(Fault::Readback(Stage::P, ReadFault::Denied));
    let outcome = lab.apply(&plan);
    for text in [
        format!("{outcome:?}"),
        format!("{:?}", lab.journal),
        format!("{plan:?}"),
    ] {
        assert!(!text.contains(marker));
        assert!(!text.contains("C:\\Users\\Secret"));
    }
    assert_eq!(
        FilterValue::parse(&RegistryRead::Unavailable),
        Err(Reason::PropertyUnavailable)
    );
}

fn failure(stage: Stage, fault: Fault, calls: usize, changed: usize, class: Classification) {
    let (plan, mut lab) = plan();
    lab.fault = Some(fault);
    let outcome = lab.apply(&plan);
    assert_eq!(outcome.classification, class);
    oracle(&lab, calls);
    assert_eq!(lab.changes.len(), changed);
    let index = MUTATION_STAGES.iter().position(|s| *s == stage).unwrap();
    assert_eq!(outcome.confirmed, MUTATION_STAGES[..index]);
    let before_calls = lab.calls.clone();
    let before_changes = lab.changes.clone();
    let resumed = lab.restart_read_only(&plan);
    assert_eq!(resumed.classification, Classification::Unknown);
    assert_eq!(lab.calls, before_calls);
    assert_eq!(lab.changes, before_changes);
}
macro_rules! failures {
    ($stage:ident,$n:expr,$a:ident,$b0:ident,$b1:ident,$c0:ident,$c1:ident,$d0:ident,$d1:ident,$d2:ident,$d3:ident,$d4:ident) => {
        #[test]
        fn $a() {
            failure(
                Stage::$stage,
                Fault::IntentSave(Stage::$stage),
                $n,
                $n,
                if $n == 0 {
                    Classification::Rejected
                } else {
                    Classification::ConfirmedPrefix
                },
            );
        }
        #[test]
        fn $b0() {
            failure(
                Stage::$stage,
                Fault::CrashBefore(Stage::$stage),
                $n,
                $n,
                Classification::Unknown,
            );
        }
        #[test]
        fn $b1() {
            failure(
                Stage::$stage,
                Fault::CrashAfter(Stage::$stage),
                $n + 1,
                $n + 1,
                Classification::Unknown,
            );
        }
        #[test]
        fn $c0() {
            failure(
                Stage::$stage,
                Fault::ApiError(Stage::$stage, false),
                $n + 1,
                $n,
                Classification::Unknown,
            );
        }
        #[test]
        fn $c1() {
            failure(
                Stage::$stage,
                Fault::ApiError(Stage::$stage, true),
                $n + 1,
                $n + 1,
                Classification::Unknown,
            );
        }
        #[test]
        fn $d0() {
            failure(
                Stage::$stage,
                Fault::Readback(Stage::$stage, ReadFault::Denied),
                $n + 1,
                $n + 1,
                Classification::Unknown,
            );
        }
        #[test]
        fn $d1() {
            failure(
                Stage::$stage,
                Fault::Readback(Stage::$stage, ReadFault::InvalidType),
                $n + 1,
                $n + 1,
                Classification::Unknown,
            );
        }
        #[test]
        fn $d2() {
            failure(
                Stage::$stage,
                Fault::Readback(Stage::$stage, ReadFault::Generation),
                $n + 1,
                $n + 1,
                Classification::Unknown,
            );
        }
        #[test]
        fn $d3() {
            failure(
                Stage::$stage,
                Fault::Readback(Stage::$stage, ReadFault::Different),
                $n + 1,
                $n + 1,
                Classification::Unknown,
            );
        }
        #[test]
        fn $d4() {
            failure(
                Stage::$stage,
                Fault::ConfirmSave(Stage::$stage),
                $n + 1,
                $n + 1,
                Classification::Unknown,
            );
        }
    };
}
failures!(
    P,
    0,
    r13a_p,
    r13b_p_before,
    r13b_p_after,
    r13c_p_no_effect,
    r13c_p_effect,
    r13d_p_denied,
    r13d_p_type,
    r13d_p_generation,
    r13d_p_content,
    r13d_p_journal
);
failures!(
    S,
    1,
    r13a_s,
    r13b_s_before,
    r13b_s_after,
    r13c_s_no_effect,
    r13c_s_effect,
    r13d_s_denied,
    r13d_s_type,
    r13d_s_generation,
    r13d_s_content,
    r13d_s_journal
);
failures!(
    K,
    2,
    r13a_k,
    r13b_k_before,
    r13b_k_after,
    r13c_k_no_effect,
    r13c_k_effect,
    r13d_k_denied,
    r13d_k_type,
    r13d_k_generation,
    r13d_k_content,
    r13d_k_journal
);
failures!(
    F,
    3,
    r13a_f,
    r13b_f_before,
    r13b_f_after,
    r13c_f_no_effect,
    r13c_f_effect,
    r13d_f_denied,
    r13d_f_type,
    r13d_f_generation,
    r13d_f_content,
    r13d_f_journal
);

#[test]
fn r13b_same_intent_different_fresh_observation() {
    for stage in MUTATION_STAGES {
        let (plan, mut before) = plan();
        let mut after = SyntheticLab::new(before.inventory.clone(), before.protection.clone());
        before.fault = Some(Fault::CrashBefore(stage));
        after.fault = Some(Fault::CrashAfter(stage));
        before.apply(&plan);
        after.apply(&plan);
        assert_eq!(before.journal, after.journal);
        assert_ne!(before.inventory, after.inventory);
        let b = before.restart_read_only(&plan);
        let a = after.restart_read_only(&plan);
        assert_eq!(b.classification, Classification::Unknown);
        assert_eq!(a.classification, Classification::Unknown);
        assert_ne!(b.observed_effects, a.observed_effects);
    }
}
#[test]
fn r14_v_crashes_and_lost_completion_never_resume() {
    for fault in [
        Fault::CrashDuringV,
        Fault::CrashBeforeComplete,
        Fault::CompleteSave,
    ] {
        let (plan, mut lab) = plan();
        lab.fault = Some(fault);
        assert_eq!(lab.apply(&plan).classification, Classification::Unknown);
        oracle(&lab, 4);
        let calls = lab.calls.clone();
        assert_eq!(
            lab.restart_read_only(&plan).classification,
            Classification::Unknown
        );
        lab.journal = None;
        assert_eq!(
            lab.restart_read_only(&plan).classification,
            Classification::Unknown
        );
        assert_eq!(lab.calls, calls);
    }
}

#[test]
fn r01_initial_rights_and_private_journal_fail_closed() {
    for rights in [Rights::ReadOnly, Rights::Unknown] {
        let (plan, mut lab) = plan();
        lab.rights = rights;
        assert_eq!(lab.apply(&plan).classification, Classification::Rejected);
        oracle(&lab, 0);
    }
    let (plan, mut lab) = plan();
    lab.journal_writable = false;
    assert_eq!(lab.apply(&plan).classification, Classification::Rejected);
    oracle(&lab, 0);
}

#[test]
fn r03_empty_device_location_same_hardware_uses_independent_identity() {
    let (inventory, protection, package) = fixture();
    assert_eq!(
        inventory.disks[0].hardware_ids,
        inventory.disks[1].hardware_ids
    );
    assert_eq!(
        inventory.disks[0].compatible_ids,
        inventory.disks[1].compatible_ids
    );
    assert_eq!(inventory.disks[1].location_paths, Observed::Known(vec![]));
    assert_eq!(
        make_plan(42, &inventory, &protection, &package, &package)
            .unwrap()
            .target_object(),
        T
    );
}

rejection!(
    r05_empty_build,
    |i: &mut Inventory, _: &mut ProtectedRecord, _: &mut PackageManifest| if let Observed::Known(
        c,
    ) = &mut i.context
    {
        c.build.clear();
    }
);
rejection!(
    r06_protected_upper_unavailable,
    |i: &mut Inventory, _: &mut ProtectedRecord, _: &mut PackageManifest| i.disks[0].upper =
        RegistryRead::Unavailable
);
rejection!(
    r06_target_lower_unavailable,
    |i: &mut Inventory, _: &mut ProtectedRecord, _: &mut PackageManifest| target(i).lower =
        RegistryRead::Unavailable
);
rejection!(
    r06_class_upper_unavailable,
    |i: &mut Inventory, _: &mut ProtectedRecord, _: &mut PackageManifest| i.class_upper =
        RegistryRead::Unavailable
);
rejection!(
    r06_duplicate_object_token,
    |i: &mut Inventory, _: &mut ProtectedRecord, _: &mut PackageManifest| i.disks[2].object = T
);

#[test]
fn r11_read_only_rights_can_confirm_completed_record_without_exclusion() {
    let (plan, mut lab) = plan();
    lab.apply(&plan);
    lab.rights = Rights::ReadOnly;
    lab.exclusion = Exclusion::Unknown;
    assert_eq!(
        lab.restart_read_only(&plan).classification,
        Classification::AlreadyConfirmed
    );
    oracle(&lab, 4);
}

#[test]
fn r14_restart_with_lost_record_cannot_be_a_new_apply() {
    let (plan, lab) = plan();
    let mut restarted = SyntheticLab::restart(lab.inventory, lab.protection, None);
    assert_eq!(
        restarted.apply(&plan).classification,
        Classification::Unknown
    );
    oracle(&restarted, 0);
}

#[test]
fn r04_non_ascii_exact_and_expansion_fail_closed() {
    let unicode = InstanceId::new("DEVICE\\é").unwrap();
    assert_eq!(
        InstanceId::from_selector(&unicode.selector()).unwrap(),
        unicode
    );
    assert!(!unicode.matches(&unicode));
    assert!(
        !InstanceId::new("DEVICE\\ß")
            .unwrap()
            .matches(&InstanceId::new("DEVICE\\SS").unwrap())
    );
    assert!(
        !InstanceId::new("DEVICE\\é")
            .unwrap()
            .matches(&InstanceId::new("DEVICE\\É").unwrap())
    );
    assert!(
        InstanceId::from_selector(&RawRegistryValue {
            value_type: 1,
            bytes: units_bytes([0xd800, 0])
        })
        .is_err()
    );
}

#[test]
fn r13_readback_decider_uses_snapshot_and_call_status_only() {
    let (plan, mut lab) = plan();
    let initial = lab.inventory.clone();
    lab.execute_store(StoreRequest::Package(fixture().2), true);
    let after = lab.inventory.clone();
    let assess = |snapshot, status| {
        assess_stage_readback(
            ReadbackBinding {
                plan: &plan,
                protection: &lab.protection,
                rights: Rights::SyntheticWrite,
                exclusion: Exclusion::WholeLab { epoch: 1 },
                epoch: 1,
            },
            Stage::P,
            Ok(snapshot),
            status,
        )
    };
    assert_eq!(
        assess(&initial, CallStatus::Failure).state,
        ReadbackState::Before
    );
    assert_eq!(
        assess(&after, CallStatus::Failure).state,
        ReadbackState::After
    );
    assert!(!assess(&after, CallStatus::Failure).confirmed);
    assert!(!assess(&initial, CallStatus::Success).confirmed);
    assert!(assess(&after, CallStatus::Success).confirmed);
    let mut unrelated = after.clone();
    unrelated.class_lower = RegistryRead::Unavailable;
    assert_eq!(
        assess(&unrelated, CallStatus::Success).state,
        ReadbackState::Neither
    );
}

#[test]
fn r13_same_error_and_returned_observation_ignore_hidden_store_effect() {
    for (index, stage) in MUTATION_STAGES.into_iter().enumerate() {
        for readback in [
            ReadFault::BeforeState,
            ReadFault::AfterState,
            ReadFault::Denied,
            ReadFault::InvalidType,
            ReadFault::Different,
            ReadFault::Generation,
        ] {
            let (plan, mut no_effect) = plan();
            let mut effect =
                SyntheticLab::new(no_effect.inventory.clone(), no_effect.protection.clone());
            no_effect.fault = Some(Fault::CallObservation {
                stage,
                status: CallStatus::Failure,
                effect: false,
                readback,
            });
            effect.fault = Some(Fault::CallObservation {
                stage,
                status: CallStatus::Failure,
                effect: true,
                readback,
            });
            let absent = no_effect.apply(&plan);
            let applied = effect.apply(&plan);
            assert_eq!(absent, applied);
            assert_eq!(absent.classification, Classification::Unknown);
            assert_eq!(absent.confirmed, MUTATION_STAGES[..index]);
            assert_eq!(
                absent.observed_effects,
                if readback == ReadFault::AfterState {
                    vec![stage]
                } else {
                    vec![]
                }
            );
            assert_eq!(no_effect.journal, effect.journal);
            assert_eq!(no_effect.calls, effect.calls);
            assert_eq!(no_effect.changes.len(), index);
            assert_eq!(effect.changes.len(), index + 1);
            oracle(&no_effect, index + 1);
            oracle(&effect, index + 1);
        }
    }
}

#[test]
fn r13_success_with_before_readback_is_contradictory_not_confirmed() {
    for (index, stage) in MUTATION_STAGES.into_iter().enumerate() {
        let mut outcomes = vec![];
        for effect in [false, true] {
            let (plan, mut lab) = plan();
            lab.fault = Some(Fault::CallObservation {
                stage,
                status: CallStatus::Success,
                effect,
                readback: ReadFault::BeforeState,
            });
            let outcome = lab.apply(&plan);
            assert_eq!(outcome.classification, Classification::Unknown);
            assert_eq!(outcome.confirmed, MUTATION_STAGES[..index]);
            assert!(outcome.observed_effects.is_empty());
            oracle(&lab, index + 1);
            assert_eq!(lab.changes.len(), index + usize::from(effect));
            outcomes.push(outcome);
        }
        assert_eq!(outcomes[0], outcomes[1]);
    }
}

#[test]
fn r13_readback_decider_requires_fresh_protection_rights_and_exclusion() {
    let (plan, mut lab) = plan();
    lab.execute_store(StoreRequest::Package(fixture().2), true);
    for (rights, exclusion) in [
        (Rights::ReadOnly, Exclusion::WholeLab { epoch: 1 }),
        (Rights::Unknown, Exclusion::WholeLab { epoch: 1 }),
        (Rights::SyntheticWrite, Exclusion::WholeLab { epoch: 2 }),
        (Rights::SyntheticWrite, Exclusion::Unknown),
    ] {
        let result = assess_stage_readback(
            ReadbackBinding {
                plan: &plan,
                protection: &lab.protection,
                rights,
                exclusion,
                epoch: 1,
            },
            Stage::P,
            Ok(&lab.inventory),
            CallStatus::Success,
        );
        assert_eq!(result.state, ReadbackState::Neither);
        assert!(!result.confirmed);
    }
    let mut stale_protection = lab.protection.clone();
    stale_protection.os.serial.push_str("-changed");
    let binding = ReadbackBinding {
        plan: &plan,
        protection: &stale_protection,
        rights: Rights::SyntheticWrite,
        exclusion: Exclusion::WholeLab { epoch: 1 },
        epoch: 1,
    };
    let result = assess_stage_readback(binding, Stage::P, Ok(&lab.inventory), CallStatus::Success);
    assert_eq!(result.state, ReadbackState::Neither);
    assert!(!result.confirmed);
}

#[test]
fn r03_t1_capacity_other_known_values_refused() {
    for bytes in [0, 1, (8 << 30) - 1, (8 << 30) + 1, 7 << 30, 64 << 30] {
        let (mut inventory, protection, package) = fixture();
        target(&mut inventory).capacity_bytes = Observed::Known(bytes);
        assert_rejected(inventory, protection, package);
    }
}
#[test]
fn r03_t1_capacity_unknown_refused() {
    let (mut inventory, protection, package) = fixture();
    target(&mut inventory).capacity_bytes = Observed::Unknown;
    assert_rejected(inventory, protection, package);
}
#[test]
fn r03_capacity_cannot_replace_strong_identity() {
    let (mut inventory, protection, package) = fixture();
    target(&mut inventory).capacity_bytes = Observed::Known(7 << 30);
    inventory.disks[2].capacity_bytes = Observed::Known(8 << 30);
    inventory.disks[2].number = 22;
    inventory.disks[2].label = "T".into();
    assert_rejected(inventory, protection, package);
}
#[test]
fn r04_non_ascii_protected_or_observed_ids_refused() {
    for object in [A, T, U] {
        let (mut inventory, mut protection, package) = fixture();
        let id = InstanceId::new("SYNTHETIC\\DISK\\é").unwrap();
        inventory
            .disks
            .iter_mut()
            .find(|disk| disk.object == object)
            .unwrap()
            .identity
            .instance = id.clone();
        if object == A {
            protection.os.instance = id.clone();
        }
        if object == T {
            protection.target.instance = id;
        }
        assert_rejected(inventory, protection, package);
    }
}
#[test]
fn r03_non_ascii_case_alias_cannot_hide_collision() {
    let (mut inventory, mut protection, package) = fixture();
    let id = InstanceId::new("SYNTHETIC\\DISK\\TÉ").unwrap();
    target(&mut inventory).identity.instance = id.clone();
    protection.target.instance = id;
    inventory.disks[2].identity.instance = InstanceId::new("synthetic\\disk\\té").unwrap();
    assert_rejected(inventory, protection, package);
}
#[test]
fn r04_inadmissible_pnp_characters_refused() {
    for suffix in [
        "bad,alias",
        "bad alias",
        "bad\talias",
        "bad\nalias",
        "bad\u{7f}alias",
    ] {
        for object in [T, U] {
            let (mut inventory, mut protection, package) = fixture();
            let id = InstanceId::new(&format!("SYNTHETIC\\DISK\\{suffix}")).unwrap();
            inventory
                .disks
                .iter_mut()
                .find(|disk| disk.object == object)
                .unwrap()
                .identity
                .instance = id.clone();
            if object == T {
                protection.target.instance = id;
            }
            assert_rejected(inventory, protection, package);
        }
    }
    let (mut inventory, mut protection, package) = fixture();
    target(&mut inventory).identity.instance = InstanceId(String::new());
    protection.target.instance = InstanceId(String::new());
    assert_rejected(inventory, protection, package);
}
#[test]
fn r04_pnp_admission_199_and_200_separate_from_codec() {
    for length in [199, 200] {
        let (mut inventory, mut protection, package) = fixture();
        let id = InstanceId::new(&format!("SYNTHETIC\\DISK\\{}", "a".repeat(length - 15))).unwrap();
        assert_eq!(id.as_str().len(), length);
        assert_eq!(InstanceId::from_selector(&id.selector()).unwrap(), id);
        target(&mut inventory).identity.instance = id.clone();
        protection.target.instance = id;
        if length == 199 {
            assert_eq!(
                make_plan(42, &inventory, &protection, &package, &package)
                    .unwrap()
                    .target_object(),
                T
            );
        } else {
            assert_rejected(inventory, protection, package);
        }
    }
}
#[test]
fn r05_empty_expected_class_or_stack_refused() {
    for kind in 0..5 {
        let (mut inventory, mut protection, package) = fixture();
        match kind {
            0 => {
                protection.expected_class.clear();
            }
            1 => {
                protection.expected_class = " ".into();
            }
            2 => {
                protection.expected_stack.clear();
            }
            3 => {
                protection.expected_stack = vec!["partmgr".into(), "".into(), "stornvme".into()];
            }
            _ => {
                protection.expected_stack = vec!["partmgr".into(), " ".into(), "stornvme".into()];
            }
        }
        for disk in inventory
            .disks
            .iter_mut()
            .filter(|disk| [A, T].contains(&disk.object))
        {
            disk.class = Observed::Known(protection.expected_class.clone());
            disk.stack = Observed::Known(protection.expected_stack.clone());
        }
        assert_rejected(inventory, protection, package);
    }
}
#[test]
fn r05_empty_or_unbound_function_service_refused() {
    for object in [A, T] {
        for service in ["", " ", "bad\0service", "not-in-stack"] {
            let (mut inventory, protection, package) = fixture();
            inventory
                .disks
                .iter_mut()
                .find(|disk| disk.object == object)
                .unwrap()
                .function_service = service.into();
            assert_rejected(inventory, protection, package);
        }
    }
}
#[test]
fn r09_invalid_admission_fresh_observation_stops_before_p() {
    for kind in 0..3 {
        let (plan, mut lab) = plan();
        match kind {
            0 => target(&mut lab.inventory).capacity_bytes = Observed::Unknown,
            1 => target(&mut lab.inventory).capacity_bytes = Observed::Known(7 << 30),
            _ => {
                lab.inventory.disks[2].identity.instance =
                    InstanceId::new("SYNTHETIC\\DISK\\é").unwrap()
            }
        };
        assert_eq!(lab.apply(&plan).classification, Classification::Rejected);
        assert!(lab.calls.is_empty() && lab.changes.is_empty());
    }
}

#[test]
fn r04_selector_boundaries() {
    assert!(InstanceId::new("").is_err());
    assert!(InstanceId::new("part\0hidden").is_err());
    let id = InstanceId::new(&"a".repeat(511)).unwrap();
    assert_eq!(InstanceId::from_selector(&id.selector()).unwrap(), id);
    assert!(InstanceId::new(&"a".repeat(512)).is_err());
}

#[test]
fn r06_query_failure_is_not_missing() {
    assert_eq!(
        FilterValue::parse(&RegistryRead::Missing),
        Ok(FilterValue::Missing)
    );
    assert_eq!(
        FilterValue::parse(&RegistryRead::Unavailable),
        Err(Reason::PropertyUnavailable)
    );
}
