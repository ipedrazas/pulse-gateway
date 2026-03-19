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
