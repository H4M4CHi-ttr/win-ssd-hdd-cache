//! Fail-closed inspection of ONE service-only AMD64 INF proposal, not a general
//! Windows INF interpreter. Unknown sections/directives/decorations are rejected.
//! No install, staging, signing, binding, service-start or PnP command is provided.
use cache_probe_registration::{Artifact, PackageEffect, PackageManifest};
use std::collections::BTreeMap;

pub const PROPOSAL: &str = include_str!("../fixtures/p04-service-only.inf");
const SERVICE: &str = "WinSsdHddCacheProbeP04";
const VERSION: &str = "0.1.0.0";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackageError {
    MalformedInf,
    UnreviewedEffect,
    MissingArtifact,
    ExtraArtifact,
    MissingSignedCatalog,
}

/// Borrowed bytes only; never opens a path or invokes a package API.
pub struct PackageFile<'a> {
    pub name: &'a str,
    pub bytes: &'a [u8],
}

/// An opaque STATIC source proof. It is not evidence of catalog coverage,
/// signature validity, staging-only behavior, exclusion, or Windows acceptance.
pub struct InspectedPackage {
    manifest: PackageManifest,
}
impl InspectedPackage {
    pub fn manifest(&self) -> &PackageManifest {
        &self.manifest
    }

    /// No signed release is allowlisted in this source/build-only increment.
    /// A boolean, fixture flag, CLI argument or feature cannot unlock deployment.
    pub fn deployment_review(&self) -> Result<(), PackageError> {
        Err(PackageError::MissingSignedCatalog)
    }
}

fn parse(text: &str) -> Result<BTreeMap<String, Vec<String>>, PackageError> {
    if text.len() > 16_384 || !text.is_ascii() || text.contains('\0') {
        return Err(PackageError::MalformedInf);
    }
    let mut sections = BTreeMap::<String, Vec<String>>::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') {
            continue;
        }
        // No continuations, inline comments, string expansion, includes, or
        // locale/decorated fallback parsing is silently approximated here.
        if line.starts_with('[') {
            if !line.ends_with(']') || line.len() < 3 {
                return Err(PackageError::MalformedInf);
            }
            let name = line[1..line.len() - 1].to_ascii_lowercase();
            if sections.insert(name.clone(), Vec::new()).is_some() {
                return Err(PackageError::MalformedInf);
            }
            current = Some(name);
        } else {
            let name = current.as_ref().ok_or(PackageError::MalformedInf)?;
            sections
                .get_mut(name)
                .ok_or(PackageError::MalformedInf)?
                .push(line.to_owned());
        }
    }
    Ok(sections)
}

fn allowed() -> BTreeMap<String, Vec<String>> {
    // Hand-authored allowlist, independent of the proposal fixture. Every
    // reachable section/reference is fixed; any other architecture or target
    // decoration, Manufacturer, AddReg, Include/Needs, extension/class install,
    // associated-service/start flag, trigger, coinstaller or dependency fails.
    [
        (
            "version",
            vec![
                "Signature=\"$WINDOWS NT$\"",
                "Class=System",
                "ClassGuid={4d36e97d-e325-11ce-bfc1-08002be10318}",
                "Provider=\"P04 source proof\"",
                "DriverVer=11/19/2026,0.1.0.0",
                "CatalogFile=p04-observer.cat",
                "PnpLockdown=1",
            ],
        ),
        ("defaultinstall.ntamd64", vec!["CopyFiles=P04.Copy"]),
        (
            "defaultinstall.ntamd64.services",
            vec!["AddService=WinSsdHddCacheProbeP04,0,P04.Service"],
        ),
        (
            "p04.service",
            vec![
                "DisplayName=\"P04 source proof observer\"",
                "ServiceType=1",
                "StartType=3",
                "ErrorControl=1",
                "ServiceBinary=%13%\\p04-observer.sys",
            ],
        ),
        ("destinationdirs", vec!["P04.Copy=13"]),
        ("p04.copy", vec!["p04-observer.sys"]),
        ("sourcedisksnames", vec!["1=\"P04 source proof\""]),
        ("sourcedisksfiles", vec!["p04-observer.sys=1"]),
    ]
    .into_iter()
    .map(|(name, values)| {
        (
            name.to_owned(),
            values.into_iter().map(str::to_owned).collect(),
        )
    })
    .collect()
}

pub fn inspect_inf(text: &str) -> Result<(), PackageError> {
    if parse(text)? != allowed() {
        return Err(PackageError::UnreviewedEffect);
    }
    Ok(())
}

/// Inspect the exact set of proposed artifacts and bind their actual bytes.
/// CAT/SYS may be synthetic; the return type never certifies them as signed.
pub fn inspect_package(files: &[PackageFile<'_>]) -> Result<InspectedPackage, PackageError> {
    const NAMES: [&str; 3] = ["p04-observer.inf", "p04-observer.cat", "p04-observer.sys"];
    if files.len() > 3 {
        return Err(PackageError::ExtraArtifact);
    }
    let mut artifacts = Vec::new();
    for name in NAMES {
        let matches: Vec<_> = files.iter().filter(|file| file.name == name).collect();
        if matches.len() != 1 {
            return Err(PackageError::MissingArtifact);
        }
        let file = matches[0];
        if file.bytes.is_empty() {
            return Err(PackageError::MissingArtifact);
        }
        if name.ends_with(".inf") {
            inspect_inf(std::str::from_utf8(file.bytes).map_err(|_| PackageError::MalformedInf)?)?;
        }
        artifacts.push(Artifact {
            name: name.to_owned(),
            sha256: sha256(file.bytes),
        });
    }
    if files.len() != 3 {
        return Err(PackageError::ExtraArtifact);
    }
    Ok(InspectedPackage {
        manifest: PackageManifest {
            service: SERVICE.to_owned(),
            version: VERSION.to_owned(),
            artifacts,
            sections: vec![
                PackageEffect::StageOnly,
                PackageEffect::DemandKernelService {
                    name: SERVICE.to_owned(),
                    binary: "p04-observer.sys".to_owned(),
                },
                PackageEffect::CopyFile {
                    name: "p04-observer.sys".to_owned(),
                },
            ],
        },
    })
}

// Hash actual supplied bytes with the pinned RustCrypto implementation.
pub(crate) fn sha256(bytes: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes).into()
}

#[cfg(test)]
mod tests;
