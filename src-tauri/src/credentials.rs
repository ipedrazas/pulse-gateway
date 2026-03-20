use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use keyring::Entry;

const SERVICE: &str = "dev.andcake.pulsegw";

/// Try keyring first, fall back to file-based storage.
/// macOS Keychain can be inaccessible in dev mode or sandboxed contexts.
fn entry(key: &str) -> Result<Entry, String> {
    Entry::new(SERVICE, key).map_err(|e| format!("Keyring error: {e}"))
}

fn fallback_path() -> PathBuf {
    let dir = dirs_next().unwrap_or_else(|| PathBuf::from("."));
    dir.join("env_secrets.json")
}

fn dirs_next() -> Option<PathBuf> {
    // Use the same config dir that Tauri would use
    let home = std::env::var("HOME").ok()?;
    let dir = PathBuf::from(home)
        .join("Library")
        .join("Application Support")
        .join("dev.andcake.pulsegw");
    fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

fn load_fallback() -> HashMap<String, String> {
    let path = fallback_path();
    match fs::read_to_string(&path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
        Err(_) => HashMap::new(),
    }
}

fn save_fallback(map: &HashMap<String, String>) {
    let path = fallback_path();
    if let Ok(data) = serde_json::to_string_pretty(map) {
        let _ = fs::write(&path, data);
    }
}

pub fn store_value(key: &str, value: &str) -> Result<(), String> {
    // Try keyring first
    if let Ok(e) = entry(key) {
        if e.set_password(value).is_ok() {
            // Verify the write by reading back
            if let Ok(readback) = e.get_password() {
                if readback == value {
                    return Ok(());
                }
            }
        }
    }

    // Fallback to file
    let mut map = load_fallback();
    map.insert(key.to_string(), value.to_string());
    save_fallback(&map);
    Ok(())
}

pub fn get_value(key: &str) -> Result<String, String> {
    // Try keyring first
    if let Ok(e) = entry(key) {
        if let Ok(val) = e.get_password() {
            return Ok(val);
        }
    }

    // Fallback to file
    let map = load_fallback();
    map.get(key)
        .cloned()
        .ok_or_else(|| format!("No value stored for '{key}'"))
}

pub fn has_value(key: &str) -> bool {
    get_value(key).is_ok()
}

pub fn delete_value(key: &str) {
    // Delete from both
    if let Ok(e) = entry(key) {
        let _ = e.delete_credential();
    }
    let mut map = load_fallback();
    if map.remove(key).is_some() {
        save_fallback(&map);
    }
}

/// Load secrets from a JSON file at the given path.
#[cfg(test)]
fn load_from_path(path: &std::path::Path) -> HashMap<String, String> {
    match fs::read_to_string(path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
        Err(_) => HashMap::new(),
    }
}

/// Save secrets to a JSON file at the given path.
#[cfg(test)]
fn save_to_path(path: &std::path::Path, map: &HashMap<String, String>) {
    if let Ok(data) = serde_json::to_string_pretty(map) {
        let _ = fs::write(path, data);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn service_name() {
        assert_eq!(SERVICE, "dev.andcake.pulsegw");
    }

    #[test]
    fn entry_creation_succeeds() {
        // Should not panic for a valid key
        let result = entry("test-key");
        assert!(result.is_ok());
    }

    #[test]
    fn load_from_missing_file() {
        let path = std::path::Path::new("/tmp/nonexistent_pulse_test.json");
        let map = load_from_path(path);
        assert!(map.is_empty());
    }

    #[test]
    fn load_from_invalid_json() {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "not json").unwrap();
        let map = load_from_path(f.path());
        assert!(map.is_empty());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let f = NamedTempFile::new().unwrap();
        let path = f.path().to_path_buf();

        let mut map = HashMap::new();
        map.insert("KEY1".to_string(), "value1".to_string());
        map.insert("KEY2".to_string(), "value2".to_string());
        save_to_path(&path, &map);

        let loaded = load_from_path(&path);
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded.get("KEY1").unwrap(), "value1");
        assert_eq!(loaded.get("KEY2").unwrap(), "value2");
    }

    #[test]
    fn save_overwrite() {
        let f = NamedTempFile::new().unwrap();
        let path = f.path().to_path_buf();

        let mut map1 = HashMap::new();
        map1.insert("A".to_string(), "1".to_string());
        save_to_path(&path, &map1);

        let mut map2 = HashMap::new();
        map2.insert("B".to_string(), "2".to_string());
        save_to_path(&path, &map2);

        let loaded = load_from_path(&path);
        assert_eq!(loaded.len(), 1);
        assert!(loaded.get("A").is_none());
        assert_eq!(loaded.get("B").unwrap(), "2");
    }

    #[test]
    fn save_empty_map() {
        let f = NamedTempFile::new().unwrap();
        let path = f.path().to_path_buf();

        save_to_path(&path, &HashMap::new());
        let loaded = load_from_path(&path);
        assert!(loaded.is_empty());
    }

    #[test]
    fn load_valid_json_with_extra_fields() {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, r#"{{"KEY":"val","OTHER":"x"}}"#).unwrap();
        let map = load_from_path(f.path());
        assert_eq!(map.len(), 2);
        assert_eq!(map.get("KEY").unwrap(), "val");
    }

    #[test]
    fn load_empty_json_object() {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "{{}}").unwrap();
        let map = load_from_path(f.path());
        assert!(map.is_empty());
    }

    #[test]
    fn save_special_characters() {
        let f = NamedTempFile::new().unwrap();
        let path = f.path().to_path_buf();

        let mut map = HashMap::new();
        map.insert(
            "KEY".to_string(),
            "value with \"quotes\" and \nnewlines".to_string(),
        );
        save_to_path(&path, &map);

        let loaded = load_from_path(&path);
        assert_eq!(
            loaded.get("KEY").unwrap(),
            "value with \"quotes\" and \nnewlines"
        );
    }

    #[test]
    fn fallback_path_ends_with_json() {
        let path = fallback_path();
        assert!(path.to_string_lossy().ends_with("env_secrets.json"));
    }
}
