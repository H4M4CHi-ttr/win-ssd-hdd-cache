//! Bounded, unprivileged offline input. Not a privileged path-authorization API.
use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};

pub(super) enum ReadError {
    Io,
    TooLarge,
    Utf8,
}

pub(super) fn read_input(path: &Path, limit: usize) -> Result<String, ReadError> {
    if !ordinary_path(path) {
        return Err(ReadError::Io);
    }
    // Reject existing links/directories/special files before opening; check the
    // opened handle again. This is not a hostile path-replacement/TOCTOU defense.
    let metadata = fs::symlink_metadata(path).map_err(|_| ReadError::Io)?;
    if !metadata.is_file() {
        return Err(ReadError::Io);
    }
    if metadata.len() > limit as u64 {
        return Err(ReadError::TooLarge);
    }
    let file = File::open(path).map_err(|_| ReadError::Io)?;
    if !file.metadata().map_err(|_| ReadError::Io)?.is_file() {
        return Err(ReadError::Io);
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ReadError::Io)?;
    if bytes.len() > limit {
        return Err(ReadError::TooLarge);
    }
    String::from_utf8(bytes).map_err(|_| ReadError::Utf8)
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
            // DOS device names are not ordinary files, even with an extension.
            let Some(name) = name.to_str() else {
                return true;
            };
            let stem = name
                .split(['.', ':'])
                .next()
                .unwrap_or("")
                .trim_end_matches(' ')
                .to_ascii_uppercase();
            !matches!(
                stem.as_str(),
                "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$" | "CLOCK$"
            ) && !(stem.len() == 4
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
    fn windows_namespace_validation_does_not_open_any_path() {
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
