//! Reads the pre-M4 JSON settings/presets files so they can be imported into SQLite.

use serde::de::DeserializeOwned;
use std::{fs, io, path::Path};

/// Read `path`, returning `Ok(None)` when it doesn't exist.
pub fn read<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, String> {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text)
            .map(Some)
            .map_err(|e| format!("{} is corrupt: {e}", path.display())),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("Could not read {}: {e}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_missing_valid_and_corrupt() {
        let dir = std::env::temp_dir().join(format!("tubedeck-json-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("x.json");
        let _ = fs::remove_file(&path);
        assert_eq!(read::<Vec<u32>>(&path).unwrap(), None);
        fs::write(&path, "[1,2,3]").unwrap();
        assert_eq!(read::<Vec<u32>>(&path).unwrap(), Some(vec![1, 2, 3]));
        fs::write(&path, "{not json").unwrap();
        assert!(read::<Vec<u32>>(&path).is_err());
        let _ = fs::remove_dir_all(dir);
    }
}
