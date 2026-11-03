//! Ordinary, unprivileged offline files only; not an elevated hostile-path API.
use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};

pub enum ReadError {
    Io,
    TooLarge,
}

pub fn read_input(path: &Path, limit: usize) -> Result<Vec<u8>, ReadError> {
    if !ordinary_path(path) {
        return Err(ReadError::Io);
    }
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
    Ok(bytes)
}

#[cfg(not(windows))]
fn ordinary_path(_path: &Path) -> bool {
    true
}

#[cfg(windows)]
fn ordinary_path(path: &Path) -> bool {
    use std::path::{Component, Prefix};
    path.components().all(|component| match component {
        Component::Prefix(prefix) => {
            matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_))
        }
        Component::Normal(name) => {
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
