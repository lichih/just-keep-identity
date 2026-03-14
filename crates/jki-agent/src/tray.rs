#![cfg(feature = "tray")]
use crate::State;
use jki_core::keychain::{KeyringStore, SecretStore};
use jki_core::paths::JkiPath;
use muda::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use notify_rust::Notification;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tray_icon::{TrayIcon, TrayIconBuilder};

pub struct TrayHandler {
    _tray: TrayIcon,
    status_label: MenuItem,
    account_label: MenuItem,
    keychain_warning: MenuItem,
    unlock_biometric_item: MenuItem,
    lock_item: MenuItem,
    reload_item: MenuItem,
    open_config_item: MenuItem,
    quit_item: MenuItem,
    last_keychain_check: Mutex<(Instant, bool)>,
}

impl TrayHandler {
    pub fn new() -> (Self, Menu) {
        let menu = Menu::new();

        // --- Dashboard Section ---
        let status_label = MenuItem::new("JKI Agent: Unknown", false, None);
        let account_label = MenuItem::new("Vault: No data loaded", false, None);
        let keychain_warning = MenuItem::new("⚠️ Keychain not configured", false, None);

        // --- Session Management Section ---
        let unlock_biometric_item =
            MenuItem::new("Unlock via Biometric (TouchID/Hello)", true, None);
        let lock_item = MenuItem::new("Purge Session & Lock Vault", true, None);

        // --- Vault Operations Section ---
        let reload_item = MenuItem::new("Refresh Secrets from Disk", true, None);
        let open_config_item = MenuItem::new("Open Config Directory...", true, None);

        // --- System Section ---
        let quit_item = MenuItem::new("Quit JKI Agent", true, None);

        let _ = menu.append_items(&[
            &status_label,
            &account_label,
            &keychain_warning,
            &PredefinedMenuItem::separator(),
            &unlock_biometric_item,
            &lock_item,
            &PredefinedMenuItem::separator(),
            &reload_item,
            &open_config_item,
            &PredefinedMenuItem::separator(),
            &quit_item,
        ]);

        let icon = load_icon();

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu.clone()))
            .with_tooltip("JKI Agent - Identity Gatekeeper")
            .with_icon(icon)
            .build()
            .unwrap();

        (
            Self {
                _tray: tray,
                status_label,
                account_label,
                keychain_warning,
                unlock_biometric_item,
                lock_item,
                reload_item,
                open_config_item,
                quit_item,
                last_keychain_check: Mutex::new((Instant::now() - Duration::from_secs(60), false)),
            },
            menu,
        )
    }

    pub fn update_status(&self, state: &State) {
        let is_unlocked = state.is_unlocked();

        // 1. Update Dashboard
        if is_unlocked {
            self.status_label
                .set_text("Status: UNLOCKED (Active Session)");
            self.account_label.set_text(format!(
                "Vault: {} accounts available",
                state.account_count()
            ));
        } else {
            self.status_label.set_text("Status: LOCKED (Safe)");
            self.account_label.set_text("Vault: Metadata only");
        }

        // 2. Keychain Health Check (Cached for 10 seconds to avoid UI lag)
        let mut check_guard = self.last_keychain_check.lock().unwrap();
        let (last_check, last_result) = *check_guard;

        let keychain_ready = if last_check.elapsed() > Duration::from_secs(10) {
            let ready = KeyringStore.get_secret("jki", "master_key").is_ok();
            *check_guard = (Instant::now(), ready);
            ready
        } else {
            last_result
        };

        if keychain_ready {
            self.keychain_warning.set_text(""); // "Hide" by clearing text
            self.keychain_warning.set_enabled(false);
        } else {
            self.keychain_warning
                .set_text("⚠️ Run 'jkim master-key set --keychain'");
            self.keychain_warning.set_enabled(false); // Labels should not be clickable
        }

        // 3. Toggle interaction states
        // Crucial: Disable biometric unlock if keychain is not ready
        self.unlock_biometric_item
            .set_enabled(!is_unlocked && keychain_ready);
        self.lock_item.set_enabled(is_unlocked);
        self.reload_item.set_enabled(is_unlocked);
    }

    pub fn handle_menu_event(&self, event: MenuEvent, state: Arc<Mutex<State>>) -> bool {
        if event.id == self.unlock_biometric_item.id() {
            // 1. Heavy/Blocking operation outside the lock (System Biometric Prompt)
            let master_key_res = KeyringStore.get_secret("jki", "master_key");

            match master_key_res {
                Ok(master_key) => {
                    // 2. Short critical section for atomic state update
                    let unlock_res = {
                        let mut s = state.lock().unwrap();
                        s.unlock(master_key)
                    };

                    match unlock_res {
                        Ok(_) => {
                            let _ = Notification::new()
                                .summary("JKI Agent")
                                .body("Vault unlocked successfully via Biometric.")
                                .show();
                        }
                        Err(e) => {
                            let _ = Notification::new()
                                .summary("Unlock Failed")
                                .body(&format!("Error: {}", e))
                                .show();
                            eprintln!("Tray: Unlock failed: {}", e);
                        }
                    }
                }
                Err(e) => {
                    let err_msg = e.to_string();
                    let body =
                        if err_msg.contains("Secret not found") || err_msg.contains("not found") {
                            "Keychain not configured. Please run: jkim master-key set --keychain"
                        } else if err_msg.contains("User interaction is not allowed")
                            || err_msg.contains("canceled")
                        {
                            "Biometric authentication was cancelled."
                        } else {
                            &err_msg
                        };
                    let _ = Notification::new()
                        .summary("Biometric Failed")
                        .body(body)
                        .show();
                    eprintln!("Tray: Keychain access failed: {}", e);
                }
            }

            // Invalidate cache to re-check status after attempt
            {
                let mut check_guard = self.last_keychain_check.lock().unwrap();
                check_guard.0 = Instant::now() - Duration::from_secs(60);
            }

            // Final UI update
            let s = state.lock().unwrap();
            self.update_status(&s);
            false
        } else if event.id == self.lock_item.id() {
            {
                let mut s = state.lock().unwrap();
                let auth = match &s.vault {
                    crate::VaultState::Locked(d) => d.auth,
                    crate::VaultState::LockedPersistent(d) => d.auth,
                    crate::VaultState::Unlocked(d) => d.auth,
                };
                s.vault = crate::VaultState::Locked(crate::LockedData { auth });
                self.update_status(&s);
            }
            println!("Tray: Vault locked and memory purged");
            false
        } else if event.id == self.reload_item.id() {
            {
                let mut s = state.lock().unwrap();
                let auth = match &s.vault {
                    crate::VaultState::Locked(d) => d.auth,
                    crate::VaultState::LockedPersistent(d) => d.auth,
                    crate::VaultState::Unlocked(d) => d.auth,
                };

                // Active Reload in Tray: Reuse the logic from main.rs via a shared trigger or just re-implement
                match &s.vault {
                    crate::VaultState::Unlocked(data) => {
                        let key = data.master_key.clone();
                        let _ = s.unlock(key);
                    }
                    crate::VaultState::LockedPersistent(data) => {
                        let key = data.master_key.clone();
                        let _ = s.unlock(key);
                    }
                    crate::VaultState::Locked(_) => {
                        let has_encrypted = JkiPath::secrets_path().exists();
                        let has_plaintext = JkiPath::decrypted_secrets_path().exists();
                        if (auth == jki_core::AuthSource::Plaintext && has_plaintext)
                            || (auth == jki_core::AuthSource::Auto
                                && has_plaintext
                                && !has_encrypted)
                        {
                            let _ = s.unlock(secrecy::SecretString::from("".to_string()));
                        }
                    }
                }
                self.update_status(&s);
            }
            println!("Tray: Refresh triggered (Active Disk Re-read)");
            false
        } else if event.id == self.open_config_item.id() {
            let path = JkiPath::home_dir();
            #[cfg(target_os = "macos")]
            let _ = std::process::Command::new("open").arg(path).spawn();
            #[cfg(target_os = "windows")]
            let _ = std::process::Command::new("explorer").arg(path).spawn();
            #[cfg(target_os = "linux")]
            let _ = std::process::Command::new("xdg-open").arg(path).spawn();
            false
        } else if event.id == self.quit_item.id() {
            true // Signal to quit
        } else {
            false
        }
    }
}

fn load_icon() -> tray_icon::Icon {
    let icon_bytes = include_bytes!("../assets/icon.png");
    let image = image::load_from_memory(icon_bytes)
        .expect("Failed to load icon from assets/icon.png")
        .into_rgba8();

    let (width, height) = image.dimensions();
    let rgba = image.into_raw();

    tray_icon::Icon::from_rgba(rgba, width, height).unwrap()
}
