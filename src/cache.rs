//! Local response cache: ~/.cache/econ-cli/{cdid}.{ext}, refreshed after 24h.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

fn cache_dir() -> Option<PathBuf> {
    // HOME on unix, USERPROFILE on Windows
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    Some(PathBuf::from(home).join(".cache").join("econ-cli"))
}

fn file_name(cdid: &str, ext: &str) -> String {
    format!("{}.{ext}", cdid.to_lowercase())
}

/// Returns the cached body for `cdid` if it exists and is younger than MAX_AGE.
pub fn read_fresh(cdid: &str, ext: &str) -> Option<String> {
    read_fresh_in(&cache_dir()?, &file_name(cdid, ext), MAX_AGE)
}

/// Writes a fetched body to the cache, creating the directory if needed.
pub fn write(cdid: &str, ext: &str, body: &str) -> io::Result<()> {
    let dir = cache_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no home directory"))?;
    fs::create_dir_all(&dir)?;
    fs::write(dir.join(file_name(cdid, ext)), body)
}

fn read_fresh_in(dir: &Path, name: &str, max_age: Duration) -> Option<String> {
    let path = dir.join(name);
    let meta = fs::metadata(&path).ok()?;
    let age = meta.modified().ok()?.elapsed().ok()?;
    if age > max_age {
        return None;
    }
    fs::read_to_string(&path).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("econ-cli-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn fresh_file_is_returned_and_stale_is_rejected() {
        let dir = temp_dir();
        fs::write(dir.join("abmi.json"), "{}").unwrap();

        // within max age -> hit
        assert_eq!(
            read_fresh_in(&dir, "abmi.json", MAX_AGE),
            Some("{}".to_string())
        );
        // zero max age -> everything is stale -> miss
        assert_eq!(read_fresh_in(&dir, "abmi.json", Duration::ZERO), None);
        // missing file -> miss
        assert_eq!(read_fresh_in(&dir, "nope.json", MAX_AGE), None);

        fs::remove_dir_all(&dir).unwrap();
    }
}
