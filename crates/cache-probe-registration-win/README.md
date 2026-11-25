# P04 Windows registration source proof

This crate is a **compile/link-only proposal**, not a registration tool. It has
no apply command, feature, privileged override or public native backend. The
`registration-link-proof` executable is a build artifact: **never execute it**.
Its source retains addresses of the actual private typed wrappers without
calling them. A linked AMD64 PE must retain these imports in debug and release:

- `SetupDiCreateDeviceInfoList`, `SetupDiDestroyDeviceInfoList`,
  `SetupDiOpenDeviceInfoW`, `SetupDiGetDeviceInstanceIdW`,
  `SetupDiGetDeviceRegistryPropertyW`, `SetupDiSetDeviceRegistryPropertyW`,
  `SetupDiGetDevicePropertyW`
- `SetupCopyOEMInfW`, `SetupGetInfDriverStoreLocationW`, `SetupOpenInfFileW`,
  `SetupCloseInfFile`, `SetupInstallServicesFromInfSectionW`
- `OpenSCManagerW`, `OpenServiceW`, `CloseServiceHandle`, `QueryServiceConfigW`,
  `QueryServiceStatusEx`
- `CompareStringOrdinal`, `GetLastError`, `SetLastError`
- `RegOpenKeyExW`, `RegCreateKeyExW`, `RegQueryValueExW`, `RegSetValueExW`, `RegCloseKey`

The public `retain_link_proof` returns neither a pointer nor a capability. Native
objects and raw leaves remain private. Tests inject leaves into those same
wrappers; no test constructs the native leaf or calls a real storage/registry API.
The wrapper validates nonempty bounded UTF-16 before opening an exact devnode,
rechecks its ID using Windows ordinal case-insensitive comparison, preserves
typed Missing/empty/value/error property states, and restricts the property setter
to that devnode's UpperFilters. The only proposed edit preserves every existing
name and appends `WinSsdHddCacheProbeP04` exactly once. Readback never proves
exclusion, device generation stability, attachment or driver loading.

The legacy SetupAPI query conflates missing and corrupt data in
`ERROR_INVALID_DATA`. For that error only, the capsule probes the same devnode's
fixed `DEVPKEY_Device_UpperFilters` with `SetupDiGetDevicePropertyW`. Only explicit
`ERROR_NOT_FOUND` confirms Missing; present, malformed, denied, unknown and
insufficient-buffer results fail closed. The legacy error alone never establishes
absence.

The selector leaf addresses only the fixed P04 Parameters key and its
`TargetInstanceId` REG_SZ, preserving its 512-unit limit and exact readback. It
opens an existing Parameters key or, after opening the already existing exact P04
service, creates only that single child with default inherited security. A fresh
AddService has no Parameters child; this is part of K, not broader service/key
creation. A creation failure may have effects and is never retried here. The leaf
does not create the service, restart PnP, load a driver, enumerate devices, write
a class filter, or expose another property selector.

The private P/S/K/F port consumes the authoritative immutable core Plan, including
its exact target, package and original/expected filters. The same generic port is
instantiated with fake leaves in tests and retained with native leaves in the PE.
It does not reimplement target selection and never returns a live apply capability.

P calls only `SetupCopyOEMInfW` with `SPOST_PATH`, null optional media location and
`SP_COPY_NOOVERWRITE`. It refuses existing/unconfirmed outcomes, validates the
returned published INF and Driver Store location, and reads/hashes the published
INF plus the store's exact INF/CAT/SYS byte set. Source bytes are independently
rechecked. The fixed service must be absent before and after P; no installation
section, class installer or device binding API is called by P.

S opens that exact store INF as System/WIN4, processes only
`DefaultInstall.NTamd64.Services` with flags zero, then closes the INF. The entire
INF is already allowlisted with one kernel service, demand start, AddService flags
zero and no deletion/start directives. A preexisting service is rejected. SCM
handles have query rights only; config/status readback requires kernel type,
demand start, ErrorControl=1, stopped state, no group/dependencies, the exact
expected image path and the actual image SHA-256. SCM inspection returns metadata
only and never opens the configured ImagePath. Only after config/name comparison
succeeds does a separate leaf hash the fixed approved receipt image. A conflicting
service, including a device/UNC/ADS path in its metadata, causes zero image-file
open/hash attempts. Any query failure, reboot
request, unknown path spelling (including unsupported `%13%` expansion), wrong
image, loaded state or partial outcome stops before K/F. No start/stop/delete
service API is imported.

## Static package scope

`fixtures/p04-service-only.inf` is an unsigned AMD64-only primitive/service
proposal. No CAT or SYS is shipped here. `inspect_inf` parses the entire bounded
ASCII input into sections and compares all entries against a separately written
allowlist, including every reference. It rejects unsupported syntax, unknown or
duplicate sections, every other architecture/build decoration, Manufacturer and
hardware matching, Include/Needs, additional copies, AddReg/class filters,
extension INFs, function-driver association, immediate-start flags, start types
other than demand, dependencies, triggers and coinstallers. This deliberately
does not implement a permissive general INF interpreter.

`inspect_package` requires exactly the named INF/CAT/SYS byte set and binds each
actual artifact's SHA-256 using pinned RustCrypto `sha2`. Synthetic CAT/SYS bytes may exercise static inspection;
they **never** establish signing or catalog membership. `deployment_review` always
returns `MissingSignedCatalog`, including after successful source inspection.
The shipped INF alone fails package inspection with `MissingArtifact`.

The INF's DefaultInstall service/copy sections describe potential installation
effects. The private fixed APIs above provide compile/link and fake-effect proof;
they do not prove that this unsigned proposal is accepted, that Driver Store
materialization and `%13%` service preparation succeed on a target Windows build,
or that external writers/PnP and first load are excluded. No deployment/signing/
installation commands are supplied. Source/store path/hash checks are not an
atomic trust boundary against replacement. There is no native journal/exclusion/
signature authority and no public route to the private port. `real_apply_gate`
remains unconditionally unavailable; the fixed native calls are never executed
in source/build tests. These limitations keep actual deployment closed even when
the fake P/S/K/F trace and linked imports match.

All source/store byte reads first require a bounded ordinary absolute DOS-drive
path. UNC, Win32 device/extended and NT namespaces, alternate streams, reserved
DOS names, dot traversal and ambiguous trailing dots/spaces are rejected before
filesystem metadata or content access. Every ancestor must be a directory and
every final input a regular file; reparse points and metadata failures reject
before a content open. The final open uses `FILE_FLAG_OPEN_REPARSE_POINT`, denies
concurrent write/delete sharing, and checks handle metadata again before reading.
These checks do **not** prove race-free ancestor custody, drive mappings, hard-link
ownership, or stability between successive API calls. The private proof has no
trusted path-custody authority; real apply remains closed on those grounds too.

## Validation boundary

The numeric boundary audit against the real SDK headers used by the coordinator
distinguishes **SPDRP_UPPERFILTERS=17 (0x11)** from SPDRP_BUSTYPEGUID=19 and the
unified UpperFilters property's **pid=19**. The raw-leaf regression observes the
numeric arguments received by both query and setter and compares them with the
independent literal 17; it does not derive its oracle from the Rust constant.

The same SDK audit confirmed these other fixed values; their distinct meanings
are preserved:

| Boundary | SDK values checked |
|---|---|
| INF staging/services | NOOVERWRITE=8, SPOST_PATH=1, WIN4=2, DriverStore DIRID=13, service flags=0 (no STARTSERVICE=0x800) |
| Service configuration/status | kernel type=1, demand start=3, normal error control=1; stopped **state**=1; status-query **level**=0; process-status size=36 |
| SCM query rights | manager CONNECT=1; service QUERY_CONFIG=1 plus QUERY_STATUS=4 |
| Registry | REG_SZ=1, REG_MULTI_SZ=7; QUERY_VALUE=1, SET_VALUE=2, CREATE_SUB_KEY=4; nonvolatile=0, disposition new/existing=1/2; sign-extended HKLM=0x80000002 |
| File checks | reparse attribute=0x400; OPEN_REPARSE_POINT=0x200000; read sharing=1; MAX_PATH=260 |
| Comparison/errors | CSTR_EQUAL=2; success=0, missing file=2, invalid data=13, service absent=1060, property absent=1168, reboot-required=3010 |

The real-header `host/user_sdk_layouts.c` compilation independently checks ABI
layouts and SDK macro assumptions; runtime native behavior remains untested.

Coordinator integration commands are `cargo test -p cache-probe-registration-win
--locked`, `cargo clippy -p cache-probe-registration-win --all-targets --locked --
-D warnings`, and debug/release `cargo build -p cache-probe-registration-win --bin
registration-link-proof --locked` (add `--release` for release). Inspect linked PE
imports and hash the artifacts; do not run the proof executable. Tests execute
only synthetic byte/leaf code. Workspace and cross-OS checks belong to the
coordinator's B01/B02/B04/B05/B06 evidence record.

The ABI and restrictive proposal were checked against Microsoft documentation:
[exact devnode open](https://learn.microsoft.com/en-us/windows/win32/api/setupapi/nf-setupapi-setupdiopendeviceinfow),
[property query](https://learn.microsoft.com/en-us/windows/win32/api/setupapi/nf-setupapi-setupdigetdeviceregistrypropertyw),
[property setter](https://learn.microsoft.com/en-us/windows/win32/api/setupapi/nf-setupapi-setupdisetdeviceregistrypropertyw),
[primitive INF requirements](https://learn.microsoft.com/en-us/windows-hardware/drivers/develop/creating-a-primitive-driver),
and [AddService flags](https://learn.microsoft.com/en-us/windows-hardware/drivers/install/inf-addservice-directive).
The P/S calls follow [INF copy restrictions](https://learn.microsoft.com/en-us/windows/win32/api/setupapi/nf-setupapi-setupcopyoeminfw),
[Driver Store location readback](https://learn.microsoft.com/en-us/windows/win32/api/setupapi/nf-setupapi-setupgetinfdriverstorelocationw),
and [exact service-section processing](https://learn.microsoft.com/en-us/windows/win32/api/setupapi/nf-setupapi-setupinstallservicesfrominfsectionw).
This is an unsafe FFI boundary requiring independent review, not a runtime or
hardware qualification claim.
