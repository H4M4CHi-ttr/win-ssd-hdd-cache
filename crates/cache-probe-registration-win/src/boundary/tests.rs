use super::*;

pub(super) struct Fake {
    pub(super) id: Vec<u16>,
    property: RegistryRead,
    selected: Option<RawRegistryValue>,
    pub(super) calls: Vec<&'static str>,
    pub(super) effects: Vec<(u32, Vec<u8>)>,
    failed_write: bool,
    corrupt_readback: bool,
    selector_wrong_type: bool,
}
impl Fake {
    pub(super) fn new() -> Self {
        Self {
            id: "SYNTHETIC\\T1\0".encode_utf16().collect(),
            property: RegistryRead::Missing,
            selected: None,
            calls: Vec::new(),
            effects: Vec::new(),
            failed_write: false,
            corrupt_readback: false,
            selector_wrong_type: false,
        }
    }
}
impl Leaves for Fake {
    fn create_set(&mut self) -> Result<usize, Error> {
        self.calls.push("create");
        Ok(19)
    }
    fn destroy_set(&mut self, set: usize) {
        assert_eq!(set, 19);
        self.calls.push("destroy");
    }
    fn open_exact(&mut self, set: usize, id: &[u16], data: &mut DevInfo) -> Result<(), Error> {
        assert_eq!(set, 19);
        assert_eq!(id, self.id);
        assert_eq!(data.cb_size, std::mem::size_of::<DevInfo>() as u32);
        self.calls.push("open");
        data.dev_inst = 41;
        Ok(())
    }
    fn instance(&mut self, set: usize, data: &mut DevInfo) -> Result<Vec<u16>, Error> {
        assert_eq!((set, data.dev_inst), (19, 41));
        self.calls.push("instance");
        Ok(self.id.clone())
    }
    fn equal_id(&mut self, a: &[u16], b: &[u16]) -> Result<bool, Error> {
        // Synthetic ASCII fixture only; native uses CompareStringOrdinal.
        Ok(String::from_utf16(a)
            .unwrap()
            .eq_ignore_ascii_case(&String::from_utf16(b).unwrap()))
    }
    fn filters(&mut self, set: usize, data: &mut DevInfo) -> Result<Property, Error> {
        assert_eq!((set, data.dev_inst), (19, 41));
        self.calls.push("get-upper");
        match &self.property {
            RegistryRead::Missing => Ok(Property::Missing),
            RegistryRead::Unavailable => Err(Error::Query),
            RegistryRead::Value(v) => Ok(Property::Value(v.value_type, v.bytes.clone())),
        }
    }
    fn set_filters(&mut self, set: usize, data: &mut DevInfo, bytes: &[u8]) -> Result<(), Error> {
        assert_eq!((set, data.dev_inst), (19, 41));
        self.calls.push("set-upper");
        // Independent effect store records what was actually addressed/written.
        self.effects.push((SPDRP_UPPERFILTERS, bytes.to_vec()));
        self.property = RegistryRead::Value(RawRegistryValue {
            value_type: if self.corrupt_readback {
                REG_SZ
            } else {
                REG_MULTI_SZ
            },
            bytes: bytes.to_vec(),
        });
        if self.failed_write {
            Err(Error::WriteUnknown)
        } else {
            Ok(())
        }
    }
    fn open_selector_key(&mut self) -> Result<usize, Error> {
        self.calls.push("key-open");
        Ok(31)
    }
    fn close_key(&mut self, key: usize) {
        assert_eq!(key, 31);
        self.calls.push("key-close");
    }
    fn selector(&mut self, key: usize) -> Result<Property, Error> {
        assert_eq!(key, 31);
        self.calls.push("selector-get");
        let value = self.selected.as_ref().unwrap();
        Ok(Property::Value(
            if self.selector_wrong_type {
                REG_MULTI_SZ
            } else {
                value.value_type
            },
            value.bytes.clone(),
        ))
    }
    fn set_selector(&mut self, key: usize, bytes: &[u8]) -> Result<(), Error> {
        assert_eq!(key, 31);
        self.calls.push("selector-set");
        self.selected = Some(RawRegistryValue {
            value_type: REG_SZ,
            bytes: bytes.to_vec(),
        });
        Ok(())
    }
}

#[test]
fn r04_empty_nul_and_overlong_never_call_open() {
    for raw in [vec![0], vec![], vec![65, 0, 66, 0], vec![65], vec![65; 513]] {
        let mut fake = Fake::new();
        assert!(open(&mut fake, &raw).is_err());
        assert!(fake.calls.is_empty());
    }
}

#[test]
fn b02_amd64_struct_layout_matches_sdk() {
    assert_eq!(std::mem::size_of::<Guid>(), 16);
    #[cfg(target_pointer_width = "64")]
    {
        assert_eq!(std::mem::size_of::<DevInfo>(), 32);
        assert_eq!(std::mem::offset_of!(DevInfo, reserved), 24);
    }
}

#[test]
fn r06_actual_wrapper_preserves_order_case_and_exact_target() {
    let mut fake = Fake::new();
    // Hand-encoded expected initial bytes: MiX\0Other\0\0.
    fake.property = RegistryRead::Value(RawRegistryValue {
        value_type: 7,
        bytes: vec![
            77, 0, 105, 0, 88, 0, 0, 0, 79, 0, 116, 0, 104, 0, 101, 0, 114, 0, 0, 0, 0, 0,
        ],
    });
    let id = fake.id.clone();
    {
        let mut device = open(&mut fake, &id).unwrap();
        device
            .write_filters(
                &FilterValue::Names(vec!["MiX".into(), "Other".into()]),
                &FilterValue::Names(vec!["MiX".into(), "Other".into(), SERVICE.into()]),
            )
            .unwrap();
    }
    let expected: Vec<u8> = "MiX\0Other\0WinSsdHddCacheProbeP04\0\0"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    assert_eq!(fake.effects, vec![(17, expected)]);
    assert_eq!(fake.calls.iter().filter(|&&c| c == "set-upper").count(), 1);
    assert_eq!(fake.calls.last(), Some(&"destroy"));
}

#[test]
fn r06_missing_empty_and_unavailable_remain_distinct() {
    assert_eq!(
        parse_filters(Property::Missing).unwrap(),
        FilterValue::Missing
    );
    assert_eq!(
        parse_filters(Property::Value(7, vec![0, 0])).unwrap(),
        FilterValue::Names(vec![])
    );
    let mut fake = Fake::new();
    fake.property = RegistryRead::Unavailable;
    let id = fake.id.clone();
    assert_eq!(
        open(&mut fake, &id).unwrap().read_filters(),
        Err(Error::Query)
    );
    assert!(fake.effects.is_empty());
}

#[test]
fn r06_malformed_property_never_issues_write() {
    for (kind, bytes) in [
        (1, vec![0, 0]),
        (7, vec![0]),
        (7, vec![65, 0, 0, 0]),
        (7, vec![0, 0, 65, 0, 0, 0]),
    ] {
        let mut fake = Fake::new();
        fake.property = RegistryRead::Value(RawRegistryValue {
            value_type: kind,
            bytes,
        });
        let id = fake.id.clone();
        assert!(
            open(&mut fake, &id)
                .unwrap()
                .write_filters(
                    &FilterValue::Missing,
                    &FilterValue::Names(vec![SERVICE.into()])
                )
                .is_err()
        );
        assert!(fake.effects.is_empty());
    }
}

#[test]
fn r09_changed_identity_stops_before_mutation_and_closes_set() {
    let mut fake = Fake::new();
    let id = fake.id.clone();
    {
        let mut device = open(&mut fake, &id).unwrap();
        device.leaves.id = "SYNTHETIC\\OTHER\0".encode_utf16().collect();
        assert_eq!(
            device.write_filters(
                &FilterValue::Missing,
                &FilterValue::Names(vec![SERVICE.into()])
            ),
            Err(Error::Changed)
        );
    }
    assert!(fake.effects.is_empty());
    assert_eq!(fake.calls.last(), Some(&"destroy"));
}

#[test]
fn r13_error_after_effect_is_unknown_without_retry_or_restore() {
    let mut fake = Fake::new();
    fake.failed_write = true;
    let id = fake.id.clone();
    assert_eq!(
        open(&mut fake, &id).unwrap().write_filters(
            &FilterValue::Missing,
            &FilterValue::Names(vec![SERVICE.into()])
        ),
        Err(Error::WriteUnknown)
    );
    assert_eq!(fake.effects.len(), 1);
    assert_eq!(fake.calls.iter().filter(|&&c| c == "set-upper").count(), 1);
}

#[test]
fn r13_success_with_wrong_readback_is_not_success() {
    let mut fake = Fake::new();
    fake.corrupt_readback = true;
    let id = fake.id.clone();
    assert!(
        open(&mut fake, &id)
            .unwrap()
            .write_filters(
                &FilterValue::Missing,
                &FilterValue::Names(vec![SERVICE.into()])
            )
            .is_err()
    );
    assert_eq!(fake.effects.len(), 1);
    assert_eq!(fake.calls.last(), Some(&"destroy"));
}

#[test]
fn r04_selector_uses_reg_sz_and_exact_readback_and_one_close() {
    let mut fake = Fake::new();
    let id = InstanceId::new("SYNTHETIC\\T1").unwrap();
    selector_roundtrip(&mut fake, &id).unwrap();
    assert_eq!(fake.selected.unwrap().value_type, 1);
    assert_eq!(
        fake.calls,
        vec!["key-open", "selector-set", "selector-get", "key-close"]
    );
}

#[test]
fn r04_selector_wrong_type_is_rejected_and_key_is_closed() {
    let mut fake = Fake::new();
    fake.selector_wrong_type = true;
    assert_eq!(
        selector_roundtrip(&mut fake, &InstanceId::new("SYNTHETIC\\T1").unwrap()),
        Err(Error::Readback)
    );
    assert_eq!(fake.calls.last(), Some(&"key-close"));
}

#[test]
fn r07_arbitrary_reorder_duplicate_or_other_service_is_never_written() {
    for (before, after) in [
        (
            FilterValue::Names(vec!["Other".into()]),
            FilterValue::Names(vec![SERVICE.into(), "Other".into()]),
        ),
        (
            FilterValue::Missing,
            FilterValue::Names(vec!["OtherService".into()]),
        ),
        (
            FilterValue::Names(vec![SERVICE.to_ascii_lowercase()]),
            FilterValue::Names(vec![SERVICE.to_ascii_lowercase(), SERVICE.into()]),
        ),
        (
            FilterValue::Missing,
            FilterValue::Names(vec![SERVICE.into(), SERVICE.into()]),
        ),
    ] {
        let mut fake = Fake::new();
        let id = fake.id.clone();
        assert_eq!(
            open(&mut fake, &id).unwrap().write_filters(&before, &after),
            Err(Error::InvalidInput)
        );
        assert!(fake.effects.is_empty());
    }
}

#[test]
fn r04_utf16_exact_without_trim_or_multichar_case_expansion() {
    assert!(exact_id(&"SYNTHETIC\\😀\0".encode_utf16().collect::<Vec<_>>()).is_ok());
    assert!(exact_id(&[0xd800, 0]).is_err());
    let mut fake = Fake::new();
    assert!(
        fake.equal_id(
            &"SYNTHETIC\\ä".encode_utf16().collect::<Vec<_>>(),
            &"SYNTHETIC\\ä".encode_utf16().collect::<Vec<_>>()
        )
        .unwrap()
    );
    assert!(
        !fake
            .equal_id(
                &"SYNTHETIC\\ß".encode_utf16().collect::<Vec<_>>(),
                &"SYNTHETIC\\SS".encode_utf16().collect::<Vec<_>>()
            )
            .unwrap()
    );
    assert!(
        !fake
            .equal_id(
                &"SYNTHETIC\\T1".encode_utf16().collect::<Vec<_>>(),
                &" SYNTHETIC\\T1".encode_utf16().collect::<Vec<_>>()
            )
            .unwrap()
    );
}

#[test]
fn r06_native_query_result_mapping_rejects_ambiguous_errors_and_truncation() {
    for error in [13, 5, 122, 1167] {
        assert!(matches!(
            queried_filters(Err(error), 7, 2, vec![0, 0]),
            Err(Error::Query)
        ));
    }
    for (kind, required, bytes) in [
        (1, 2, vec![0, 0]),
        (7, 4, vec![0, 0]),
        (7, 1, vec![0]),
        (7, 4, vec![65, 0, 0, 0]),
    ] {
        assert!(matches!(
            queried_filters(Ok(()), kind, required, bytes),
            Err(Error::Query)
        ));
    }
    assert_eq!(
        parse_filters(queried_filters(Ok(()), 7, 2, vec![0, 0, 99, 99]).unwrap()).unwrap(),
        FilterValue::Names(vec![])
    );
}

#[test]
fn r06_unified_property_only_explicit_not_found_confirms_missing() {
    assert!(matches!(
        confirmed_absence(13, Err(1168)),
        Ok(Property::Missing)
    ));
    for (legacy, modern) in [
        (13, Ok(())),
        (13, Err(13)),
        (13, Err(122)),
        (13, Err(5)),
        (5, Err(1168)),
    ] {
        assert!(matches!(
            confirmed_absence(legacy, modern),
            Err(Error::Query)
        ));
    }
    assert_eq!(UPPER_FILTERS_KEY.pid, 19);
    assert_eq!(UPPER_FILTERS_KEY.guid.data1, 0xa45c254e);
}

#[test]
fn r06_raw_sdk_upperfilters_read_write_property_numbers() {
    struct Raw {
        calls: Vec<(&'static str, u32)>,
        absent: bool,
    }
    impl RawPropertyLeaves for Raw {
        fn read_registry(
            &mut self,
            set: usize,
            data: &mut DevInfo,
            property: u32,
        ) -> RawPropertyRead {
            assert_eq!(set, 91);
            assert_eq!(data.cb_size, 32);
            self.calls.push(("read", property));
            RawPropertyRead {
                status: if self.absent { Err(13) } else { Ok(()) },
                kind: 7,
                required: 2,
                bytes: vec![0, 0],
            }
        }
        fn query_unified(
            &mut self,
            set: usize,
            _: &mut DevInfo,
            key: &DevPropKey,
        ) -> Result<(), u32> {
            assert_eq!(set, 91);
            self.calls.push(("unified", key.pid));
            assert_eq!(
                (
                    key.guid.data1,
                    key.guid.data2,
                    key.guid.data3,
                    key.guid.data4
                ),
                (
                    0xa45c254e,
                    0xdf1c,
                    0x4efd,
                    [0x80, 0x20, 0x67, 0xd1, 0x46, 0xa8, 0x50, 0xe0]
                )
            );
            Err(1168)
        }
        fn write_registry(
            &mut self,
            set: usize,
            _: &mut DevInfo,
            property: u32,
            bytes: &[u8],
        ) -> Result<(), Error> {
            assert_eq!(set, 91);
            assert_eq!(bytes, [0, 0]);
            self.calls.push(("write", property));
            Ok(())
        }
    }
    let mut raw = Raw {
        calls: vec![],
        absent: false,
    };
    let mut data = DevInfo::new();
    assert_eq!(
        parse_filters(read_upper(&mut raw, 91, &mut data).unwrap()).unwrap(),
        FilterValue::Names(vec![])
    );
    write_upper(&mut raw, 91, &mut data, &[0, 0]).unwrap();
    // Independent SDK oracle: SPDRP_UPPERFILTERS is decimal17, NOT the unified
    // DEVPROPKEY pid19 (nor SPDRP_BUSTYPEGUID decimal19).
    assert_eq!(raw.calls, vec![("read", 17), ("write", 17)]);
    raw.calls.clear();
    raw.absent = true;
    assert!(matches!(
        read_upper(&mut raw, 91, &mut data),
        Ok(Property::Missing)
    ));
    assert_eq!(raw.calls, vec![("read", 17), ("unified", 19)]);
}
