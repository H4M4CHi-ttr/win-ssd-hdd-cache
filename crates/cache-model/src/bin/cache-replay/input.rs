//! Explicit local file reader, not a privileged path-authorization API.
use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};

pub(super) fn read(path: &Path, limit: usize) -> Result<String, &'static str> {
    if !ordinary_path(path) {
        return Err("INPUT_IO");
    }
    // Reject existing final links, directories and special files before opening
    // and verify the opened handle. Not a hostile replacement/TOCTOU defense.
    let metadata = fs::symlink_metadata(path).map_err(|_| "INPUT_IO")?;
    if !metadata.is_file() {
        return Err("INPUT_IO");
    }
    if metadata.len() > limit as u64 {
        return Err("INPUT_TOO_LARGE");
    }
    let file = File::open(path).map_err(|_| "INPUT_IO")?;
    if !file.metadata().map_err(|_| "INPUT_IO")?.is_file() {
        return Err("INPUT_IO");
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "INPUT_IO")?;
    if bytes.len() > limit {
        return Err("INPUT_TOO_LARGE");
    }
    String::from_utf8(bytes).map_err(|_| "INPUT_UTF8")
}

#[cfg(not(windows))]
fn ordinary_path(_path: &Path) -> bool {
    true
}

#[cfg(windows)]
fn ordinary_path(path: &Path) -> bool {
    use std::path::{Component, Prefix};
    path.components().all(|component| match component {
        Component::Prefix(p) => matches!(p.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)),
        Component::Normal(name) => {
            let Some(name) = name.to_str() else {
                return true;
            };
            let stem = name
                .split('.')
                .next()
                .unwrap_or("")
                .trim_end_matches(' ')
                .to_ascii_uppercase();
            !name.contains(':')
                && !matches!(
                    stem.as_str(),
                    "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$" | "CLOCK$"
                )
                && !(stem.len() == 4
                    && (stem.starts_with("COM") || stem.starts_with("LPT"))
                    && stem.as_bytes()[3].is_ascii_digit())
                && !["COM¹", "COM²", "COM³", "LPT¹", "LPT²", "LPT³"].contains(&stem.as_str())
        }
        _ => true,
    })
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn namespaces_and_dos_names_are_rejected_before_opening_any_path() {
        for path in [
            r"\\.\PhysicalDrive0",
            r"\\.\pipe\anything",
            r"\\?\GLOBALROOT\Device\Harddisk0",
            r"\\server\share\input.json",
            r"\\?\UNC\server\share\input.json",
            r"C:\NUL.json",
            r"nul",
            r"COM1",
            r"LPT²",
            r"C:\file:stream",
        ] {
            assert!(!ordinary_path(Path::new(path)), "{path}");
        }
        for path in [
            r"C:\data\input.json",
            r"\\?\C:\data\input.json",
            r"relative\日本語.json",
        ] {
            assert!(ordinary_path(Path::new(path)), "{path}");
        }
    }
}
