//! Tiny helpers for JSON files in the app config dir, written atomically.

use serde::{Serialize, de::DeserializeOwned};
use std::{fs, io, path::Path};

/// Read `path`, returning `Ok(None)` when it doesn't exist yet.
pub fn read<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, String> {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text)
            .map(Some)
            .map_err(|e| format!("{} is corrupt: {e}", path.display())),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("Could not read {}: {e}", path.display())),
    }
}

/// Write via a temp file + rename so a crash never leaves a half-written file.
pub fn write<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    fs::write(&tmp, text).map_err(|e| format!("Could not write {}: {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| format!("Could not replace {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_missing() {
        let dir = std::env::temp_dir().join(format!("tubedeck-test-{}", std::process::id()));
        let path = dir.join("x.json");
        let _ = fs::remove_file(&path);
        assert_eq!(read::<Vec<u32>>(&path).unwrap(), None);
        write(&path, &vec![1u32, 2, 3]).unwrap();
        assert_eq!(read::<Vec<u32>>(&path).unwrap(), Some(vec![1, 2, 3]));
        fs::write(&path, "{not json").unwrap();
        assert!(read::<Vec<u32>>(&path).is_err());
        let _ = fs::remove_dir_all(dir);
    }
}
