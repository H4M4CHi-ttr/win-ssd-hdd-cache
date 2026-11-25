//! Private ownership capsule. All calls are synchronous, user-mode/PASSIVE_LEVEL.
//! No aliases outlive a call: each Vec owns its buffer and each device/key has one
//! owner. Neither handles nor the raw leaf table are exported or Send/Sync.
//! Native constructors are referenced for LINK ONLY, never called by this crate.
use cache_probe_registration::{FilterValue, InstanceId, RawRegistryValue, RegistryRead};

const SPDRP_UPPERFILTERS: u32 = 0x11;
const REG_SZ: u32 = 1;
const REG_MULTI_SZ: u32 = 7;
const MAX_PROPERTY: usize = 65_536;
const SERVICE: &str = "WinSsdHddCacheProbeP04";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Error {
    InvalidInput,
    Query,
    Changed,
    WriteUnknown,
    Readback,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

#[repr(C)]
struct DevPropKey {
    guid: Guid,
    pid: u32,
}
const UPPER_FILTERS_KEY: DevPropKey = DevPropKey {
    guid: Guid {
        data1: 0xa45c254e,
        data2: 0xdf1c,
        data3: 0x4efd,
        data4: [0x80, 0x20, 0x67, 0xd1, 0x46, 0xa8, 0x50, 0xe0],
    },
    pid: 19,
};

#[repr(C)]
struct DevInfo {
    cb_size: u32,
    class_guid: Guid,
    dev_inst: u32,
    reserved: usize,
}
impl DevInfo {
    fn new() -> Self {
        Self {
            cb_size: std::mem::size_of::<Self>() as u32,
            class_guid: Guid::default(),
            dev_inst: 0,
            reserved: 0,
        }
    }
}

/// A missing property is distinct from any failure or malformed property.
enum Property {
    Missing,
    Value(u32, Vec<u8>),
}

struct RawPropertyRead {
    status: Result<(), u32>,
    kind: u32,
    required: u32,
    bytes: Vec<u8>,
}
trait RawPropertyLeaves {
    fn read_registry(&mut self, set: usize, data: &mut DevInfo, property: u32) -> RawPropertyRead;
    fn query_unified(
        &mut self,
        set: usize,
        data: &mut DevInfo,
        key: &DevPropKey,
    ) -> Result<(), u32>;
    fn write_registry(
        &mut self,
        set: usize,
        data: &mut DevInfo,
        property: u32,
        bytes: &[u8],
    ) -> Result<(), Error>;
}
fn read_upper(
    raw: &mut impl RawPropertyLeaves,
    set: usize,
    data: &mut DevInfo,
) -> Result<Property, Error> {
    let result = raw.read_registry(set, data, SPDRP_UPPERFILTERS);
    if result.status == Err(13) {
        return confirmed_absence(13, raw.query_unified(set, data, &UPPER_FILTERS_KEY));
    }
    queried_filters(result.status, result.kind, result.required, result.bytes)
}
fn write_upper(
    raw: &mut impl RawPropertyLeaves,
    set: usize,
    data: &mut DevInfo,
    bytes: &[u8],
) -> Result<(), Error> {
    parse_filters(Property::Value(REG_MULTI_SZ, bytes.to_vec()))?;
    raw.write_registry(set, data, SPDRP_UPPERFILTERS, bytes)
}

// Deliberately private. A fixture cannot provide an exclusion token or construct
// a real backend through a public generic apply function.
trait Leaves {
    fn create_set(&mut self) -> Result<usize, Error>;
    fn destroy_set(&mut self, set: usize);
    fn open_exact(&mut self, set: usize, id: &[u16], data: &mut DevInfo) -> Result<(), Error>;
    fn instance(&mut self, set: usize, data: &mut DevInfo) -> Result<Vec<u16>, Error>;
    fn equal_id(&mut self, a: &[u16], b: &[u16]) -> Result<bool, Error>;
    fn filters(&mut self, set: usize, data: &mut DevInfo) -> Result<Property, Error>;
    fn set_filters(&mut self, set: usize, data: &mut DevInfo, bytes: &[u8]) -> Result<(), Error>;
    fn open_selector_key(&mut self) -> Result<usize, Error>;
    fn close_key(&mut self, key: usize);
    fn selector(&mut self, key: usize) -> Result<Property, Error>;
    fn set_selector(&mut self, key: usize, bytes: &[u8]) -> Result<(), Error>;
}

struct Device<'a, L: Leaves> {
    leaves: &'a mut L,
    set: usize,
    data: DevInfo,
    id: Vec<u16>,
}
impl<L: Leaves> Drop for Device<'_, L> {
    fn drop(&mut self) {
        self.leaves.destroy_set(self.set);
    }
}

fn exact_id(raw: &[u16]) -> Result<Vec<u16>, Error> {
    if raw.len() < 2
        || raw.len() > 512
        || raw.last() != Some(&0)
        || raw[..raw.len() - 1].contains(&0)
    {
        return Err(Error::InvalidInput);
    }
    let text = String::from_utf16(&raw[..raw.len() - 1]).map_err(|_| Error::InvalidInput)?;
    InstanceId::new(&text).map_err(|_| Error::InvalidInput)?;
    Ok(raw.to_vec())
}

fn open<'a, L: Leaves>(leaves: &'a mut L, raw: &[u16]) -> Result<Device<'a, L>, Error> {
    let id = exact_id(raw)?; // Empty IDs must never reach OpenDeviceInfo/root selection.
    let set = leaves.create_set()?;
    let mut device = Device {
        leaves,
        set,
        data: DevInfo::new(),
        id,
    };
    device
        .leaves
        .open_exact(set, &device.id, &mut device.data)?;
    device.reconfirm()?;
    Ok(device)
}

fn parse_filters(property: Property) -> Result<FilterValue, Error> {
    let raw = match property {
        Property::Missing => RegistryRead::Missing,
        Property::Value(value_type, bytes) => {
            if bytes.len() > MAX_PROPERTY {
                return Err(Error::Query);
            }
            RegistryRead::Value(RawRegistryValue { value_type, bytes })
        }
    };
    FilterValue::parse(&raw).map_err(|_| Error::Query)
}

fn queried_filters(
    status: Result<(), u32>,
    kind: u32,
    required: u32,
    mut bytes: Vec<u8>,
) -> Result<Property, Error> {
    // Legacy ERROR_INVALID_DATA (13) conflates absent and corrupt properties.
    status.map_err(|_| Error::Query)?;
    if kind != REG_MULTI_SZ || required as usize > bytes.len() || bytes.len() > MAX_PROPERTY {
        return Err(Error::Query);
    }
    bytes.truncate(required as usize);
    parse_filters(Property::Value(kind, bytes.clone()))?;
    Ok(Property::Value(kind, bytes))
}

fn confirmed_absence(legacy_error: u32, modern_status: Result<(), u32>) -> Result<Property, Error> {
    if legacy_error == 13 && modern_status == Err(1168) {
        Ok(Property::Missing)
    } else {
        Err(Error::Query)
    }
}

fn encode_filters(value: &FilterValue) -> Result<Vec<u8>, Error> {
    let FilterValue::Names(names) = value else {
        return Err(Error::InvalidInput);
    };
    let mut units = Vec::new();
    for name in names {
        if name.is_empty() || name.contains('\0') {
            return Err(Error::InvalidInput);
        }
        units.extend(name.encode_utf16());
        units.push(0);
    }
    units.push(0); // Empty REG_MULTI_SZ is one terminating empty string.
    let bytes: Vec<u8> = units.into_iter().flat_map(u16::to_le_bytes).collect();
    if bytes.len() > MAX_PROPERTY {
        return Err(Error::InvalidInput);
    }
    Ok(bytes)
}

impl<L: Leaves> Device<'_, L> {
    fn reconfirm(&mut self) -> Result<(), Error> {
        let observed = exact_id(&self.leaves.instance(self.set, &mut self.data)?)?;
        if !self.leaves.equal_id(
            &self.id[..self.id.len() - 1],
            &observed[..observed.len() - 1],
        )? {
            return Err(Error::Changed);
        }
        Ok(())
    }
    fn read_filters(&mut self) -> Result<FilterValue, Error> {
        self.reconfirm()?;
        parse_filters(self.leaves.filters(self.set, &mut self.data)?)
    }
    fn write_filters(&mut self, before: &FilterValue, after: &FilterValue) -> Result<(), Error> {
        let mut expected = match before {
            FilterValue::Missing => Vec::new(),
            FilterValue::Names(names) => names.clone(),
        };
        if expected
            .iter()
            .any(|name| name.eq_ignore_ascii_case(SERVICE))
        {
            return Err(Error::InvalidInput);
        }
        expected.push(SERVICE.to_owned());
        if *after != FilterValue::Names(expected) {
            return Err(Error::InvalidInput);
        }
        let bytes = encode_filters(after)?;
        if self.read_filters()? != *before {
            return Err(Error::Changed);
        }
        // Comparison is NOT exclusion/CAS. This wrapper is compile-only until a
        // separately reviewed exclusion/deployment mechanism exists.
        self.leaves
            .set_filters(self.set, &mut self.data, &bytes)
            .map_err(|_| Error::WriteUnknown)?;
        if self.read_filters()? != *after {
            return Err(Error::Readback);
        }
        Ok(())
    }
}

fn selector_roundtrip<L: Leaves>(leaves: &mut L, id: &InstanceId) -> Result<(), Error> {
    struct Key<'a, L: Leaves> {
        leaves: &'a mut L,
        key: usize,
    }
    impl<L: Leaves> Drop for Key<'_, L> {
        fn drop(&mut self) {
            self.leaves.close_key(self.key);
        }
    }
    let value = id.selector();
    let key = leaves.open_selector_key()?;
    let guard = Key { leaves, key };
    guard
        .leaves
        .set_selector(key, &value.bytes)
        .map_err(|_| Error::WriteUnknown)?;
    let Property::Value(value_type, bytes) = guard.leaves.selector(key)? else {
        return Err(Error::Readback);
    };
    let read = InstanceId::from_selector(&RawRegistryValue { value_type, bytes })
        .map_err(|_| Error::Readback)?;
    let left: Vec<u16> = id.as_str().encode_utf16().collect();
    let right: Vec<u16> = read.as_str().encode_utf16().collect();
    if !guard.leaves.equal_id(&left, &right)? {
        return Err(Error::Readback);
    }
    Ok(())
}

#[cfg(all(windows, target_arch = "x86_64"))]
mod native {
    use super::*;
    use std::{ffi::c_void, ptr};
    type Handle = *mut c_void;

    #[link(name = "setupapi")]
    unsafe extern "system" {
        fn SetupDiCreateDeviceInfoList(class: *const Guid, window: Handle) -> Handle;
        fn SetupDiDestroyDeviceInfoList(set: Handle) -> i32;
        fn SetupDiOpenDeviceInfoW(
            set: Handle,
            id: *const u16,
            window: Handle,
            flags: u32,
            data: *mut DevInfo,
        ) -> i32;
        fn SetupDiGetDeviceInstanceIdW(
            set: Handle,
            data: *mut DevInfo,
            id: *mut u16,
            size: u32,
            required: *mut u32,
        ) -> i32;
        fn SetupDiGetDeviceRegistryPropertyW(
            set: Handle,
            data: *mut DevInfo,
            property: u32,
            kind: *mut u32,
            bytes: *mut u8,
            size: u32,
            required: *mut u32,
        ) -> i32;
        fn SetupDiGetDevicePropertyW(
            set: Handle,
            data: *mut DevInfo,
            key: *const DevPropKey,
            kind: *mut u32,
            bytes: *mut u8,
            size: u32,
            required: *mut u32,
            flags: u32,
        ) -> i32;
        fn SetupDiSetDeviceRegistryPropertyW(
            set: Handle,
            data: *mut DevInfo,
            property: u32,
            bytes: *const u8,
            size: u32,
        ) -> i32;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetLastError() -> u32;
        fn CompareStringOrdinal(
            a: *const u16,
            a_len: i32,
            b: *const u16,
            b_len: i32,
            ignore_case: i32,
        ) -> i32;
    }
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegOpenKeyExW(
            key: Handle,
            subkey: *const u16,
            options: u32,
            access: u32,
            result: *mut Handle,
        ) -> i32;
        fn RegCreateKeyExW(
            key: Handle,
            subkey: *const u16,
            reserved: u32,
            class: *mut u16,
            options: u32,
            access: u32,
            security: *const c_void,
            result: *mut Handle,
            disposition: *mut u32,
        ) -> i32;
        fn RegCloseKey(key: Handle) -> i32;
        fn RegQueryValueExW(
            key: Handle,
            name: *const u16,
            reserved: *mut u32,
            kind: *mut u32,
            bytes: *mut u8,
            size: *mut u32,
        ) -> i32;
        fn RegSetValueExW(
            key: Handle,
            name: *const u16,
            reserved: u32,
            kind: u32,
            bytes: *const u8,
            size: u32,
        ) -> i32;
    }

    // No public constructor or trait implementation escape hatch.
    pub(super) struct Native(std::marker::PhantomData<std::rc::Rc<()>>);
    impl RawPropertyLeaves for Native {
        fn read_registry(
            &mut self,
            set: usize,
            data: &mut DevInfo,
            property: u32,
        ) -> RawPropertyRead {
            let mut bytes = vec![0u8; MAX_PROPERTY];
            let (mut kind, mut required) = (0, 0);
            // SAFETY: live exact set/element, exclusive bounded output; the
            // shared typed wrapper chooses the actual numeric property argument.
            let ok = unsafe {
                SetupDiGetDeviceRegistryPropertyW(
                    set as Handle,
                    data,
                    property,
                    &mut kind,
                    bytes.as_mut_ptr(),
                    MAX_PROPERTY as u32,
                    &mut required,
                )
            };
            let status = if ok == 0 {
                Err(unsafe { GetLastError() })
            } else {
                Ok(())
            };
            RawPropertyRead {
                status,
                kind,
                required,
                bytes,
            }
        }
        fn query_unified(
            &mut self,
            set: usize,
            data: &mut DevInfo,
            key: &DevPropKey,
        ) -> Result<(), u32> {
            let (mut kind, mut needed) = (0, 0);
            // SAFETY: same live exact devnode, fixed key from shared wrapper,
            // null output/size0 probes existence, required flags zero.
            let present = unsafe {
                SetupDiGetDevicePropertyW(
                    set as Handle,
                    data,
                    key,
                    &mut kind,
                    ptr::null_mut(),
                    0,
                    &mut needed,
                    0,
                )
            };
            if present == 0 {
                Err(unsafe { GetLastError() })
            } else {
                Ok(())
            }
        }
        fn write_registry(
            &mut self,
            set: usize,
            data: &mut DevInfo,
            property: u32,
            bytes: &[u8],
        ) -> Result<(), Error> {
            // SAFETY: typed wrapper validated bounded REG_MULTI_SZ and selected
            // the fixed devnode property. No caller-supplied selector is public.
            if unsafe {
                SetupDiSetDeviceRegistryPropertyW(
                    set as Handle,
                    data,
                    property,
                    bytes.as_ptr(),
                    bytes.len() as u32,
                )
            } == 0
            {
                Err(Error::WriteUnknown)
            } else {
                Ok(())
            }
        }
    }
    impl Leaves for Native {
        fn create_set(&mut self) -> Result<usize, Error> {
            // SAFETY: null class creates an empty set, no enumeration or install.
            let set = unsafe { SetupDiCreateDeviceInfoList(ptr::null(), ptr::null_mut()) };
            if set.is_null() || set as isize == -1 {
                Err(Error::Query)
            } else {
                Ok(set as usize)
            }
        }
        fn destroy_set(&mut self, set: usize) {
            // SAFETY: exclusively owned successful set, dropped once.
            unsafe {
                SetupDiDestroyDeviceInfoList(set as Handle);
            }
        }
        fn open_exact(&mut self, set: usize, id: &[u16], data: &mut DevInfo) -> Result<(), Error> {
            exact_id(id)?;
            // SAFETY: validated nonempty NUL-terminated ID, correct live struct;
            // flags zero neither cancel removal nor inherit class driver lists.
            if unsafe {
                SetupDiOpenDeviceInfoW(set as Handle, id.as_ptr(), ptr::null_mut(), 0, data)
            } == 0
            {
                Err(Error::Query)
            } else {
                Ok(())
            }
        }
        fn instance(&mut self, set: usize, data: &mut DevInfo) -> Result<Vec<u16>, Error> {
            let mut units = vec![0u16; 512];
            let mut required = 0;
            // SAFETY: live set/element, exclusive 512-unit output buffer. No
            // truncated success accepted and no widening retry after overflow.
            let ok = unsafe {
                SetupDiGetDeviceInstanceIdW(
                    set as Handle,
                    data,
                    units.as_mut_ptr(),
                    512,
                    &mut required,
                )
            };
            if ok == 0 || !(2..=512).contains(&required) {
                return Err(Error::Query);
            }
            units.truncate(required as usize);
            exact_id(&units)
        }
        fn equal_id(&mut self, a: &[u16], b: &[u16]) -> Result<bool, Error> {
            if a.len() > 511 || b.len() > 511 {
                return Err(Error::InvalidInput);
            }
            // SAFETY: exact live UTF-16 slices with explicit bounded lengths.
            match unsafe {
                CompareStringOrdinal(a.as_ptr(), a.len() as i32, b.as_ptr(), b.len() as i32, 1)
            } {
                0 => Err(Error::Query),
                2 => Ok(true),
                _ => Ok(false),
            }
        }
        fn filters(&mut self, set: usize, data: &mut DevInfo) -> Result<Property, Error> {
            read_upper(self, set, data)
        }
        fn set_filters(
            &mut self,
            set: usize,
            data: &mut DevInfo,
            bytes: &[u8],
        ) -> Result<(), Error> {
            write_upper(self, set, data, bytes)
        }
        fn open_selector_key(&mut self) -> Result<usize, Error> {
            let path: Vec<u16> =
                "SYSTEM\\CurrentControlSet\\Services\\WinSsdHddCacheProbeP04\\Parameters\0"
                    .encode_utf16()
                    .collect();
            let mut key = ptr::null_mut();
            // SAFETY: fixed service path, existing key only, QUERY_VALUE|SET_VALUE.
            let result = unsafe {
                RegOpenKeyExW((-2147483646isize) as Handle, path.as_ptr(), 0, 3, &mut key)
            };
            if result == 0 && !key.is_null() {
                return Ok(key as usize);
            }
            if result != 2 {
                return Err(Error::Query);
            }
            // A new service has no Parameters child. Open the EXISTING exact
            // service first; never create Services or the service itself here.
            let service_path: Vec<u16> =
                "SYSTEM\\CurrentControlSet\\Services\\WinSsdHddCacheProbeP04\0"
                    .encode_utf16()
                    .collect();
            let mut service = ptr::null_mut();
            if unsafe {
                RegOpenKeyExW(
                    (-2147483646isize) as Handle,
                    service_path.as_ptr(),
                    0,
                    4,
                    &mut service,
                )
            } != 0
                || service.is_null()
            {
                return Err(Error::Query);
            }
            struct Parent(Handle);
            impl Drop for Parent {
                fn drop(&mut self) {
                    unsafe {
                        RegCloseKey(self.0);
                    }
                }
            }
            let parent = Parent(service);
            let child: Vec<u16> = "Parameters\0".encode_utf16().collect();
            let mut disposition = 0;
            // SAFETY: owned existing service key, fixed single child, no class,
            // default inherited security, nonvolatile, QUERY_VALUE|SET_VALUE only.
            let created = unsafe {
                RegCreateKeyExW(
                    parent.0,
                    child.as_ptr(),
                    0,
                    ptr::null_mut(),
                    0,
                    3,
                    ptr::null(),
                    &mut key,
                    &mut disposition,
                )
            };
            if created != 0 || key.is_null() {
                return Err(Error::WriteUnknown);
            }
            if !matches!(disposition, 1 | 2) {
                unsafe {
                    RegCloseKey(key);
                }
                return Err(Error::WriteUnknown);
            }
            Ok(key as usize)
        }
        fn close_key(&mut self, key: usize) {
            // SAFETY: successful exclusively owned key, closed once.
            unsafe {
                RegCloseKey(key as Handle);
            }
        }
        fn selector(&mut self, key: usize) -> Result<Property, Error> {
            let name: Vec<u16> = "TargetInstanceId\0".encode_utf16().collect();
            let mut bytes = vec![0u8; 1024];
            let (mut kind, mut size) = (0, 1024);
            // SAFETY: fixed name and exclusive bounded synchronous output.
            let result = unsafe {
                RegQueryValueExW(
                    key as Handle,
                    name.as_ptr(),
                    ptr::null_mut(),
                    &mut kind,
                    bytes.as_mut_ptr(),
                    &mut size,
                )
            };
            if result == 2 {
                return Ok(Property::Missing);
            }
            if result != 0 || size > 1024 {
                return Err(Error::Query);
            }
            bytes.truncate(size as usize);
            Ok(Property::Value(kind, bytes))
        }
        fn set_selector(&mut self, key: usize, bytes: &[u8]) -> Result<(), Error> {
            InstanceId::from_selector(&RawRegistryValue {
                value_type: REG_SZ,
                bytes: bytes.to_vec(),
            })
            .map_err(|_| Error::InvalidInput)?;
            let name: Vec<u16> = "TargetInstanceId\0".encode_utf16().collect();
            // SAFETY: validated bounded REG_SZ, fixed value name, owned key.
            if unsafe {
                RegSetValueExW(
                    key as Handle,
                    name.as_ptr(),
                    0,
                    REG_SZ,
                    bytes.as_ptr(),
                    bytes.len() as u32,
                )
            } != 0
            {
                Err(Error::WriteUnknown)
            } else {
                Ok(())
            }
        }
    }

    // Retain actual typed wrappers, NOT an unrelated list of import addresses.
    // The optimizer must preserve each function body because its address escapes
    // through black_box; no pointer or capability is returned to the caller.
    pub(super) fn retain() {
        std::hint::black_box(open::<Native> as *const ());
        std::hint::black_box(Device::<Native>::read_filters as *const ());
        std::hint::black_box(Device::<Native>::write_filters as *const ());
        std::hint::black_box(selector_roundtrip::<Native> as *const ());
        std::hint::black_box(stages::registration_prefix::<Native> as *const ());
    }
}

#[cfg(all(windows, target_arch = "x86_64"))]
pub(super) fn retain_link_proof() {
    native::retain();
}

mod stages;
#[cfg(test)]
mod tests;
