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
    let dir = dirs_next()
        .unwrap_or_else(|| PathBuf::from("."));
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
    eprintln!("[credentials::store] key='{key}', value_len={}", value.len());

    // Try keyring first
    match entry(key) {
        Ok(e) => {
            eprintln!("[credentials::store] keyring Entry created OK for '{key}'");
            match e.set_password(value) {
                Ok(()) => {
                    eprintln!("[credentials::store] keyring set_password OK for '{key}'");
                    // Verify the write by reading back immediately
                    match e.get_password() {
                        Ok(readback) => {
                            if readback == value {
                                eprintln!("[credentials::store] keyring readback VERIFIED for '{key}' (len={})", readback.len());
                                return Ok(());
                            } else {
                                eprintln!("[credentials::store] keyring readback MISMATCH for '{key}': wrote len={}, read len={}", value.len(), readback.len());
                            }
                        }
                        Err(e) => {
                            eprintln!("[credentials::store] keyring readback FAILED for '{key}': {e}");
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[credentials::store] keyring set_password FAILED for '{key}': {e}");
                }
            }
        }
        Err(e) => {
            eprintln!("[credentials::store] keyring Entry creation FAILED for '{key}': {e}");
        }
    }

    // Fallback to file
    eprintln!("[credentials::store] using file fallback for '{key}'");
    let path = fallback_path();
    eprintln!("[credentials::store] fallback path: {}", path.display());
    let mut map = load_fallback();
    map.insert(key.to_string(), value.to_string());
    save_fallback(&map);

    // Verify file write
    let verify_map = load_fallback();
    if verify_map.get(key).map(|v| v.as_str()) == Some(value) {
        eprintln!("[credentials::store] file fallback write VERIFIED for '{key}'");
    } else {
        eprintln!("[credentials::store] file fallback write FAILED verification for '{key}'");
    }

    Ok(())
}

pub fn get_value(key: &str) -> Result<String, String> {
    eprintln!("[credentials::get] key='{key}'");

    // Try keyring first
    match entry(key) {
        Ok(e) => {
            match e.get_password() {
                Ok(val) => {
                    eprintln!("[credentials::get] keyring HIT for '{key}' (len={})", val.len());
                    return Ok(val);
                }
                Err(e) => {
                    eprintln!("[credentials::get] keyring MISS for '{key}': {e}");
                }
            }
        }
        Err(e) => {
            eprintln!("[credentials::get] keyring Entry creation FAILED for '{key}': {e}");
        }
    }

    // Fallback to file
    let path = fallback_path();
    eprintln!("[credentials::get] trying file fallback at {}", path.display());
    let map = load_fallback();
    eprintln!("[credentials::get] file has {} keys: {:?}", map.len(), map.keys().collect::<Vec<_>>());
    match map.get(key) {
        Some(val) => {
            eprintln!("[credentials::get] file HIT for '{key}' (len={})", val.len());
            Ok(val.clone())
        }
        None => {
            eprintln!("[credentials::get] file MISS for '{key}' — NOT FOUND anywhere");
            Err(format!("No value stored for '{key}'"))
        }
    }
}

pub fn has_value(key: &str) -> bool {
    let result = get_value(key).is_ok();
    eprintln!("[credentials::has] key='{key}' → {result}");
    result
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
