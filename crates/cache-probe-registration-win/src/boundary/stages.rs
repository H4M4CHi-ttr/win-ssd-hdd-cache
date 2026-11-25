//! Private P/S/K/F boundary decisions consume the same immutable core Plan.
//! Neither this port nor its native implementation is exported. There is no
//! journal/exclusion/signing capability, so this is NEVER a live executor.
use super::*;
use cache_probe_registration::{
    PackageManifest, Plan,
    synthetic::{MUTATION_STAGES, Stage},
};
use std::path::{Path, PathBuf};

const SERVICE_SECTION: &str = "DefaultInstall.NTamd64.Services";
const SP_COPY_NOOVERWRITE: u32 = 8;
const MAX_FILE: u64 = 64 * 1024 * 1024;

pub(super) struct Source {
    directory: PathBuf,
}
pub(super) struct Receipt {
    published: PathBuf,
    store_inf: PathBuf,
    image: PathBuf,
}
impl Receipt {
    fn validate(&self) -> Result<(), Error> {
        let published = self
            .published
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(Error::Readback)?
            .to_ascii_lowercase();
        let digits = published
            .strip_prefix("oem")
            .and_then(|name| name.strip_suffix(".inf"))
            .ok_or(Error::Readback)?;
        if digits.is_empty()
            || !digits.bytes().all(|byte| byte.is_ascii_digit())
            || self.store_inf.file_name().and_then(|name| name.to_str()) != Some("p04-observer.inf")
            || self.image
                != self
                    .store_inf
                    .parent()
                    .ok_or(Error::Readback)?
                    .join("p04-observer.sys")
        {
            return Err(Error::Readback);
        }
        Ok(())
    }
}
#[derive(Clone, PartialEq, Eq)]
pub(super) enum ServiceView {
    Absent,
    Present {
        kind: u32,
        start: u32,
        error: u32,
        state: u32,
        image: Vec<u16>,
    },
}

pub(super) trait StageLeaves: Leaves {
    fn source_matches(&mut self, source: &Source, package: &PackageManifest) -> Result<(), Error>;
    fn service_view(&mut self) -> Result<ServiceView, Error>;
    fn approved_image_hash(&mut self, receipt: &Receipt) -> Result<[u8; 32], Error>;
    fn copy_inf(&mut self, source: &Source, copy_flags: u32) -> Result<Receipt, Error>;
    fn staged_matches(&mut self, receipt: &Receipt, package: &PackageManifest)
    -> Result<(), Error>;
    fn install_service_section(
        &mut self,
        receipt: &Receipt,
        section: &str,
        flags: u32,
    ) -> Result<(), Error>;
}

fn verified_service<L: StageLeaves>(
    leaves: &mut L,
    receipt: &Receipt,
    package: &PackageManifest,
) -> Result<(), Error> {
    receipt.validate()?;
    let ServiceView::Present {
        kind,
        start,
        error,
        state,
        image,
    } = leaves.service_view()?
    else {
        return Err(Error::Readback);
    };
    let expected_image: Vec<u16> = receipt
        .image
        .to_str()
        .ok_or(Error::InvalidInput)?
        .encode_utf16()
        .collect();
    let expected_hash = package
        .artifacts
        .iter()
        .find(|file| file.name == "p04-observer.sys")
        .ok_or(Error::InvalidInput)?
        .sha256;
    if kind != 1
        || start != 3
        || error != 1
        || state != 1
        || !leaves.equal_id(&image, &expected_image)?
    {
        return Err(Error::Readback);
    }
    if leaves.approved_image_hash(receipt)? != expected_hash {
        return Err(Error::Readback);
    }
    Ok(())
}

/// Compile-only concrete port. Core owns planning/restart/journaling decisions;
/// these leaves cannot claim those preconditions. Retaining its native body
/// proves every fixed P/S/K/F wrapper is linked, never authority to invoke it.
pub(super) fn registration_prefix<L: StageLeaves>(
    leaves: &mut L,
    plan: &Plan,
    source: &Source,
) -> Result<[Stage; 4], Error> {
    let package = plan.package();
    if package.service != SERVICE || source.directory.as_os_str().is_empty() {
        return Err(Error::InvalidInput);
    }
    leaves.source_matches(source, package)?;
    if leaves.service_view()? != ServiceView::Absent {
        return Err(Error::Changed);
    }
    let id = plan.target_instance();
    let before = FilterValue::parse(plan.original_upper()).map_err(|_| Error::InvalidInput)?;
    let after = FilterValue::parse(plan.expected_upper()).map_err(|_| Error::InvalidInput)?;
    {
        let mut device = open(
            leaves,
            &id.as_str().encode_utf16().chain([0]).collect::<Vec<_>>(),
        )?;
        if device.read_filters()? != before {
            return Err(Error::Changed);
        }
    }
    let mut receipt: Option<Receipt> = None;
    for stage in MUTATION_STAGES {
        leaves.source_matches(source, package)?;
        match stage {
            Stage::P => {
                let staged = leaves.copy_inf(source, SP_COPY_NOOVERWRITE)?;
                staged.validate()?;
                leaves.staged_matches(&staged, package)?;
                if leaves.service_view()? != ServiceView::Absent {
                    return Err(Error::Changed);
                }
                receipt = Some(staged);
            }
            Stage::S => {
                let staged = receipt.as_ref().ok_or(Error::InvalidInput)?;
                leaves.staged_matches(staged, package)?;
                if leaves.service_view()? != ServiceView::Absent {
                    return Err(Error::Changed);
                }
                leaves.install_service_section(staged, SERVICE_SECTION, 0)?;
                leaves.staged_matches(staged, package)?;
                verified_service(leaves, staged, package)?;
            }
            Stage::K => {
                let staged = receipt.as_ref().ok_or(Error::InvalidInput)?;
                leaves.staged_matches(staged, package)?;
                verified_service(leaves, staged, package)?;
                selector_roundtrip(leaves, id)?;
            }
            Stage::F => {
                let staged = receipt.as_ref().ok_or(Error::InvalidInput)?;
                leaves.staged_matches(staged, package)?;
                verified_service(leaves, staged, package)?;
                let mut device = open(
                    leaves,
                    &id.as_str().encode_utf16().chain([0]).collect::<Vec<_>>(),
                )?;
                device.write_filters(&before, &after)?;
            }
            Stage::V => return Err(Error::InvalidInput),
        }
    }
    Ok(MUTATION_STAGES)
}

#[derive(Clone, Copy)]
struct FileMetadata {
    regular: bool,
    directory: bool,
    reparse: bool,
}
trait LocalFiles {
    fn metadata(&mut self, path: &Path) -> Result<FileMetadata, Error>;
    fn read_regular(&mut self, path: &Path, maximum: u64) -> Result<Vec<u8>, Error>;
}

fn ordinary_dos_path(text: &str) -> Result<Vec<PathBuf>, Error> {
    let bytes = text.as_bytes();
    if bytes.len() < 4
        || !bytes[0].is_ascii_alphabetic()
        || bytes[1] != b':'
        || bytes[2] != b'\\'
        || text.encode_utf16().count() >= 260
        || text
            .chars()
            .any(|c| c < ' ' || matches!(c, '/' | '<' | '>' | '"' | '|' | '?' | '*'))
    {
        return Err(Error::InvalidInput);
    }
    let mut prefixes = vec![PathBuf::from(&text[..3])];
    let mut current = text[..3].to_owned();
    for component in text[3..].split('\\') {
        let stem = component
            .split('.')
            .next()
            .unwrap_or("")
            .trim_end_matches(' ')
            .to_uppercase();
        let reserved = matches!(
            stem.as_str(),
            "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
        ) || ["COM", "LPT"].iter().any(|prefix| {
            stem.strip_prefix(prefix).is_some_and(|tail| {
                matches!(
                    tail,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            })
        });
        if component.is_empty()
            || component == "."
            || component == ".."
            || component.ends_with(['.', ' '])
            || component.contains(':')
            || reserved
        {
            return Err(Error::InvalidInput);
        }
        if current.len() > 3 {
            current.push('\\');
        }
        current.push_str(component);
        prefixes.push(PathBuf::from(&current));
    }
    Ok(prefixes)
}

fn checked_read(files: &mut impl LocalFiles, path: &Path) -> Result<Vec<u8>, Error> {
    let prefixes = ordinary_dos_path(path.to_str().ok_or(Error::InvalidInput)?)?;
    for (index, prefix) in prefixes.iter().enumerate() {
        let meta = files.metadata(prefix)?;
        if meta.reparse
            || (index + 1 == prefixes.len() && !meta.regular)
            || (index + 1 < prefixes.len() && !meta.directory)
        {
            return Err(Error::Query);
        }
    }
    let bytes = files.read_regular(path, MAX_FILE)?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_FILE {
        return Err(Error::Query);
    }
    Ok(bytes)
}

#[cfg(all(windows, target_arch = "x86_64"))]
struct Files;
#[cfg(all(windows, target_arch = "x86_64"))]
impl LocalFiles for Files {
    fn metadata(&mut self, path: &Path) -> Result<FileMetadata, Error> {
        use std::os::windows::fs::MetadataExt;
        let metadata = std::fs::symlink_metadata(path).map_err(|_| Error::Query)?;
        Ok(FileMetadata {
            regular: metadata.is_file(),
            directory: metadata.is_dir(),
            reparse: metadata.file_attributes() & 0x400 != 0,
        })
    }
    fn read_regular(&mut self, path: &Path, maximum: u64) -> Result<Vec<u8>, Error> {
        use std::{
            io::Read,
            os::windows::fs::{MetadataExt, OpenOptionsExt},
        };
        // Final-component reparses are opened as reparses, never followed. Every
        // ancestor was separately checked, but replacement races remain gated.
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .custom_flags(0x0020_0000)
            .open(path)
            .map_err(|_| Error::Query)?;
        let metadata = file.metadata().map_err(|_| Error::Query)?;
        if !metadata.is_file() || metadata.file_attributes() & 0x400 != 0 {
            return Err(Error::Query);
        }
        let mut bytes = Vec::new();
        file.by_ref()
            .take(maximum + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| Error::Query)?;
        Ok(bytes)
    }
}
#[cfg(all(windows, target_arch = "x86_64"))]
fn bounded_read(path: &Path) -> Result<Vec<u8>, Error> {
    checked_read(&mut Files, path)
}
#[cfg(all(windows, target_arch = "x86_64"))]
fn inspect_directory(directory: &Path, inf: &Path, package: &PackageManifest) -> Result<(), Error> {
    let inf_bytes = bounded_read(inf)?;
    let cat = bounded_read(&directory.join("p04-observer.cat"))?;
    let sys = bounded_read(&directory.join("p04-observer.sys"))?;
    let files = [
        crate::package::PackageFile {
            name: "p04-observer.inf",
            bytes: &inf_bytes,
        },
        crate::package::PackageFile {
            name: "p04-observer.cat",
            bytes: &cat,
        },
        crate::package::PackageFile {
            name: "p04-observer.sys",
            bytes: &sys,
        },
    ];
    let inspected = crate::package::inspect_package(&files).map_err(|_| Error::InvalidInput)?;
    if inspected.manifest() != package {
        return Err(Error::Changed);
    }
    Ok(())
}
fn wide_path(path: &Path) -> Result<Vec<u16>, Error> {
    if !path.is_absolute() {
        return Err(Error::InvalidInput);
    }
    let text = path.to_str().ok_or(Error::InvalidInput)?;
    ordinary_dos_path(text)?;
    let units: Vec<u16> = text.encode_utf16().chain([0]).collect();
    if units.len() > 260 || units.len() < 2 || units[..units.len() - 1].contains(&0) {
        return Err(Error::InvalidInput);
    }
    Ok(units)
}
fn returned_path(mut units: Vec<u16>, required: u32) -> Result<PathBuf, Error> {
    if !(2..=260).contains(&required) || required as usize > units.len() {
        return Err(Error::Readback);
    }
    units.truncate(required as usize);
    if units.last() != Some(&0) || units[..units.len() - 1].contains(&0) {
        return Err(Error::Readback);
    }
    let text = String::from_utf16(&units[..units.len() - 1]).map_err(|_| Error::Readback)?;
    let path = PathBuf::from(text);
    wide_path(&path)?;
    Ok(path)
}

#[cfg(all(windows, target_arch = "x86_64"))]
mod native {
    use super::*;
    use std::{ffi::c_void, ptr};
    type Handle = *mut c_void;
    #[repr(C)]
    pub(super) struct ServiceConfig {
        kind: u32,
        start: u32,
        error: u32,
        pub(super) binary: *mut u16,
        group: *mut u16,
        tag: u32,
        dependencies: *mut u16,
        start_name: *mut u16,
        display: *mut u16,
    }
    #[link(name = "setupapi")]
    unsafe extern "system" {
        fn SetupCopyOEMInfW(
            source: *const u16,
            media: *const u16,
            media_type: u32,
            copy_style: u32,
            destination: *mut u16,
            destination_size: u32,
            required: *mut u32,
            component: *mut *mut u16,
        ) -> i32;
        fn SetupGetInfDriverStoreLocationW(
            file: *const u16,
            platform: *mut c_void,
            locale: *const u16,
            output: *mut u16,
            size: u32,
            required: *mut u32,
        ) -> i32;
        fn SetupOpenInfFileW(
            file: *const u16,
            class: *const u16,
            style: u32,
            line: *mut u32,
        ) -> Handle;
        fn SetupCloseInfFile(inf: Handle);
        fn SetupInstallServicesFromInfSectionW(inf: Handle, section: *const u16, flags: u32)
        -> i32;
    }
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn OpenSCManagerW(machine: *const u16, database: *const u16, access: u32) -> Handle;
        fn OpenServiceW(manager: Handle, name: *const u16, access: u32) -> Handle;
        fn CloseServiceHandle(service: Handle) -> i32;
        fn QueryServiceConfigW(
            service: Handle,
            config: *mut ServiceConfig,
            size: u32,
            needed: *mut u32,
        ) -> i32;
        fn QueryServiceStatusEx(
            service: Handle,
            level: i32,
            buffer: *mut u8,
            size: u32,
            needed: *mut u32,
        ) -> i32;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetLastError() -> u32;
        fn SetLastError(error: u32);
    }

    struct ScHandle(Handle);
    impl Drop for ScHandle {
        fn drop(&mut self) {
            unsafe {
                CloseServiceHandle(self.0);
            }
        }
    }
    struct InfHandle(Handle);
    impl Drop for InfHandle {
        fn drop(&mut self) {
            unsafe {
                SetupCloseInfFile(self.0);
            }
        }
    }

    fn buffer_string(
        pointer: *const u16,
        buffer: &[usize],
        optional: bool,
    ) -> Result<Vec<u16>, Error> {
        if pointer.is_null() {
            return if optional {
                Ok(vec![])
            } else {
                Err(Error::Query)
            };
        }
        let start = buffer.as_ptr() as usize;
        let end = start + std::mem::size_of_val(buffer);
        let at = pointer as usize;
        if at < start || at >= end || !at.is_multiple_of(2) {
            return Err(Error::Query);
        }
        // SAFETY: returned pointer was range/alignment checked within this live
        // aligned output allocation; bounded scan never reads past the buffer.
        let units = unsafe { std::slice::from_raw_parts(pointer, (end - at) / 2) };
        let nul = units
            .iter()
            .position(|unit| *unit == 0)
            .ok_or(Error::Query)?;
        String::from_utf16(&units[..nul]).map_err(|_| Error::Query)?;
        Ok(units[..nul].to_vec())
    }

    impl StageLeaves for super::super::native::Native {
        fn approved_image_hash(&mut self, receipt: &Receipt) -> Result<[u8; 32], Error> {
            receipt.validate()?;
            Ok(crate::package::sha256(&bounded_read(&receipt.image)?))
        }
        fn source_matches(
            &mut self,
            source: &Source,
            package: &PackageManifest,
        ) -> Result<(), Error> {
            inspect_directory(
                &source.directory,
                &source.directory.join("p04-observer.inf"),
                package,
            )
        }
        fn copy_inf(&mut self, source: &Source, copy_flags: u32) -> Result<Receipt, Error> {
            if copy_flags != SP_COPY_NOOVERWRITE {
                return Err(Error::InvalidInput);
            }
            let input = wide_path(&source.directory.join("p04-observer.inf"))?;
            let mut destination = vec![0u16; 260];
            let mut required = 0;
            // SAFETY: bounded exact source path, SPOST_PATH with null media uses
            // its directory, fixed NOOVERWRITE (no delete/replace/install flags).
            let copied = unsafe {
                SetupCopyOEMInfW(
                    input.as_ptr(),
                    ptr::null(),
                    1,
                    SP_COPY_NOOVERWRITE,
                    destination.as_mut_ptr(),
                    260,
                    &mut required,
                    ptr::null_mut(),
                )
            };
            if copied == 0 {
                return Err(Error::WriteUnknown);
            }
            let published = returned_path(destination, required)?;
            let published_w = wide_path(&published)?;
            let mut store = vec![0u16; 260];
            let mut size = 0;
            // SAFETY: reserved arguments null; bounded MAX_PATH output.
            if unsafe {
                SetupGetInfDriverStoreLocationW(
                    published_w.as_ptr(),
                    ptr::null_mut(),
                    ptr::null(),
                    store.as_mut_ptr(),
                    260,
                    &mut size,
                )
            } == 0
            {
                return Err(Error::WriteUnknown);
            }
            let store_inf = returned_path(store, size)?;
            if store_inf.file_name().and_then(|name| name.to_str()) != Some("p04-observer.inf") {
                return Err(Error::Readback);
            }
            let image = store_inf
                .parent()
                .ok_or(Error::Readback)?
                .join("p04-observer.sys");
            Ok(Receipt {
                published,
                store_inf,
                image,
            })
        }
        fn staged_matches(
            &mut self,
            receipt: &Receipt,
            package: &PackageManifest,
        ) -> Result<(), Error> {
            let expected = package
                .artifacts
                .iter()
                .find(|file| file.name == "p04-observer.inf")
                .ok_or(Error::InvalidInput)?
                .sha256;
            if crate::package::sha256(&bounded_read(&receipt.published)?) != expected {
                return Err(Error::Readback);
            }
            inspect_directory(
                receipt.store_inf.parent().ok_or(Error::Readback)?,
                &receipt.store_inf,
                package,
            )
        }
        fn install_service_section(
            &mut self,
            receipt: &Receipt,
            section: &str,
            flags: u32,
        ) -> Result<(), Error> {
            if section != SERVICE_SECTION || flags != 0 {
                return Err(Error::InvalidInput);
            }
            let path = wide_path(&receipt.store_inf)?;
            let class: Vec<u16> = "System\0".encode_utf16().collect();
            let mut line = 0;
            // SAFETY: exact store INF; require System class and WIN4 style only.
            let raw = unsafe { SetupOpenInfFileW(path.as_ptr(), class.as_ptr(), 2, &mut line) };
            if raw.is_null() || raw as isize == -1 {
                return Err(Error::Query);
            }
            let inf = InfHandle(raw);
            let section: Vec<u16> = SERVICE_SECTION.encode_utf16().chain([0]).collect();
            // SAFETY: exclusively owned live INF, exact statically allowlisted
            // Services section, flags0 never requests start, stop or deletion.
            unsafe {
                SetLastError(0);
            }
            let ok = unsafe { SetupInstallServicesFromInfSectionW(inf.0, section.as_ptr(), 0) };
            let error = unsafe { GetLastError() };
            if ok == 0 || error != 0 {
                return Err(Error::WriteUnknown);
            } // includes reboot-required; never reboot.
            Ok(())
        }
        fn service_view(&mut self) -> Result<ServiceView, Error> {
            // SAFETY: local active database, CONNECT only; no service mutation.
            let manager = unsafe { OpenSCManagerW(ptr::null(), ptr::null(), 1) };
            if manager.is_null() {
                return Err(Error::Query);
            }
            let manager = ScHandle(manager);
            let name: Vec<u16> = SERVICE.encode_utf16().chain([0]).collect();
            // SAFETY: fixed exact name, QUERY_CONFIG|QUERY_STATUS only.
            let service = unsafe { OpenServiceW(manager.0, name.as_ptr(), 5) };
            if service.is_null() {
                return if unsafe { GetLastError() } == 1060 {
                    Ok(ServiceView::Absent)
                } else {
                    Err(Error::Query)
                };
            }
            let service = ScHandle(service);
            let mut buffer = vec![0usize; 8192 / std::mem::size_of::<usize>()];
            let mut needed = 0;
            // SAFETY: exclusive aligned 8KiB output, checked live query handle.
            if unsafe {
                QueryServiceConfigW(service.0, buffer.as_mut_ptr().cast(), 8192, &mut needed)
            } == 0
            {
                return Err(Error::Query);
            }
            // SAFETY: successful query initialized fixed prefix of aligned buffer.
            let config = unsafe { &*buffer.as_ptr().cast::<ServiceConfig>() };
            let image = buffer_string(config.binary, &buffer, false)?;
            if image.is_empty()
                || !buffer_string(config.group, &buffer, true)?.is_empty()
                || !buffer_string(config.dependencies, &buffer, true)?.is_empty()
            {
                return Err(Error::Query);
            }
            // SCM data is metadata only. Never open/follow this configured path;
            // verified_service compares it before hashing the fixed receipt image.
            let mut status = [0u32; 9];
            let mut required = 0;
            // SAFETY: exclusive SERVICE_STATUS_PROCESS sized output, level0.
            if unsafe {
                QueryServiceStatusEx(service.0, 0, status.as_mut_ptr().cast(), 36, &mut required)
            } == 0
                || required > 36
            {
                return Err(Error::Query);
            }
            if status[0] != config.kind {
                return Err(Error::Query);
            }
            Ok(ServiceView::Present {
                kind: config.kind,
                start: config.start,
                error: config.error,
                state: status[1],
                image,
            })
        }
    }
}

#[cfg(test)]
mod tests;
