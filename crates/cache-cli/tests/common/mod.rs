use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

pub struct TempInput {
    root: PathBuf,
    pub path: PathBuf,
}
impl TempInput {
    pub fn new(bytes: &[u8]) -> Self {
        Self::named(OsStr::new("input space 日本語.json"), bytes)
    }
    pub fn named(name: &OsStr, bytes: &[u8]) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        for _ in 0..100 {
            let root = std::env::temp_dir().join(format!(
                "win-cache-cli-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&root) {
                Ok(()) => {
                    let path = root.join(name);
                    fs::write(&path, bytes).unwrap();
                    return Self { root, path };
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("temporary test directory: {e}"),
            }
        }
        panic!("cannot allocate unique test directory");
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
}
impl Drop for TempInput {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
pub fn run(args: &[&OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cache-cli"))
        .args(args)
        .output()
        .unwrap()
}
pub fn status(path: &Path, format: &str) -> Output {
    run(&[
        OsStr::new("status"),
        OsStr::new("--input"),
        path.as_os_str(),
        OsStr::new("--format"),
        OsStr::new(format),
    ])
}
pub fn assert_error(output: Output, exit: i32, error: &str) {
    assert_eq!(output.status.code(), Some(exit));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        format!("error: {error}\n")
    );
}
