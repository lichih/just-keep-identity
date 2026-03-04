use muda::{Menu, MenuItem, PredefinedMenuItem, MenuEvent};
use tray_icon::{TrayIcon, TrayIconBuilder};
use std::sync::{Arc, Mutex};
use crate::State;
use jki_core::paths::JkiPath;
use jki_core::keychain::{KeyringStore, SecretStore};
use notify_rust::Notification;
use std::time::{Instant, Duration};

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
        let unlock_biometric_item = MenuItem::new("Unlock via Biometric (TouchID/Hello)", true, None);
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
        let is_unlocked = state.secrets.is_some();
        
        // 1. Update Dashboard
        if is_unlocked {
            self.status_label.set_text("Status: UNLOCKED (Active Session)");
            self.account_label.set_text(format!("Vault: {} accounts available", state.account_count()));
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
            self.keychain_warning.set_text("⚠️ Run 'jkim master-key set --keychain'");
            self.keychain_warning.set_enabled(false); // Labels should not be clickable
        }
        
        // 3. Toggle interaction states
        // Crucial: Disable biometric unlock if keychain is not ready
        self.unlock_biometric_item.set_enabled(!is_unlocked && keychain_ready);
        self.lock_item.set_enabled(is_unlocked);
        self.reload_item.set_enabled(is_unlocked);
    }

    pub fn handle_menu_event(&self, event: MenuEvent, state: Arc<Mutex<State>>) -> bool {
        if event.id == self.unlock_biometric_item.id() {
            let mut s = state.lock().unwrap();
            match s.unlock_with_biometric() {
                Ok(_) => {
                    let _ = Notification::new()
                        .summary("JKI Agent")
                        .body("Vault unlocked successfully via Biometric.")
                        .show();
                },
                Err(e) => {
                    let body = if e.contains("Secret not found") {
                        "Keychain not configured. Please run: jkim master-key set --keychain"
                    } else {
                        &e
                    };
                    let _ = Notification::new()
                        .summary("Unlock Failed")
                        .body(body)
                        .show();
                    eprintln!("Tray: Biometric unlock failed: {}", e);
                }
            }
            // Invalidate cache to re-check after attempt
            {
                let mut check_guard = self.last_keychain_check.lock().unwrap();
                check_guard.0 = Instant::now() - Duration::from_secs(60);
            }
            self.update_status(&s);
            false
        } else if event.id == self.lock_item.id() {
            let mut s = state.lock().unwrap();
            s.secrets = None;
            s.master_key = None;
            s.last_unlocked = None;
            self.update_status(&s);
            println!("Tray: Vault locked and memory purged");
            false
        } else if event.id == self.reload_item.id() {
            let mut s = state.lock().unwrap();
            s.secrets = None; // Force reload on next request
            println!("Tray: Refresh requested (cache cleared)");
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
