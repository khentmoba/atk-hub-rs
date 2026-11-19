//! GUI pages for lighting, macros, music, pets, plugins, updates and drivers.

use super::{HubApp, theme};
use atk_config::Config;
use atk_features::{DriverInfo, Lighting, Macro, MacroStep, Plugin, UpdateInfo};
use std::path::PathBuf;
use std::str::FromStr;

/// State for the feature pages.
#[derive(Debug)]
pub(super) struct FeatureState {
    /// Keymap name selected on the lighting page.
    pub keymap: String,
    /// Red component.
    pub red: String,
    /// Green component.
    pub green: String,
    /// Blue component.
    pub blue: String,
    /// Brightness.
    pub brightness: String,
    /// Keymap assets found in the installed renderer bundle.
    pub keymaps: Vec<String>,
    /// Lighting status line.
    pub lighting_status: String,
    /// Macro storage path.
    pub macro_path: String,
    /// New macro name.
    pub macro_name: String,
    /// New macro keys.
    pub macro_keys: String,
    /// Loaded macros.
    pub macros: Vec<Macro>,
    /// Macro status line.
    pub macro_status: String,
    /// Music service status line.
    pub music_status: String,
    /// Desk-pet settings path.
    pub pet_path: String,
    /// Desk-pet settings document.
    pub pets: Option<Config>,
    /// Desk-pet status line.
    pub pet_status: String,
    /// Electron user-data directory.
    pub user_data: String,
    /// Installed plugins.
    pub plugins: Vec<Plugin>,
    /// Plugin status line.
    pub plugin_status: String,
    /// Plugin bundle URL.
    pub plugin_url: String,
    /// Plugin id.
    pub plugin_id: String,
    /// Plugin version.
    pub plugin_version: String,
    /// Expected plugin SHA-256.
    pub plugin_sha: String,
    /// Update check result.
    pub update: Option<UpdateInfo>,
    /// Update status line.
    pub update_status: String,
    /// Driver metadata result.
    pub driver: Option<DriverInfo>,
    /// Driver status line.
    pub driver_status: String,
    /// Expected driver archive SHA-256.
    pub driver_sha: String,
    /// Extracted driver directory.
    pub driver_extract: String,
}

impl Default for FeatureState {
    fn default() -> Self {
        let user_data = dirs_home().join("AppData").join("Roaming").join("ATK-HUB");
        Self {
            keymap: String::new(),
            red: "255".to_owned(),
            green: "255".to_owned(),
            blue: "255".to_owned(),
            brightness: "128".to_owned(),
            keymaps: Vec::new(),
            lighting_status: "No keymaps loaded".to_owned(),
            macro_path: user_data.join("macros.json").to_string_lossy().into_owned(),
            macro_name: String::new(),
            macro_keys: String::new(),
            macros: Vec::new(),
            macro_status: "No macros loaded".to_owned(),
            music_status: "Not checked".to_owned(),
            pet_path: user_data
                .join("wooden-fish.json")
                .to_string_lossy()
                .into_owned(),
            pets: None,
            pet_status: "No pet settings loaded".to_owned(),
            user_data: user_data.to_string_lossy().into_owned(),
            plugins: Vec::new(),
            plugin_status: "No plugin scan performed".to_owned(),
            plugin_url: String::new(),
            plugin_id: String::new(),
            plugin_version: "1.0".to_owned(),
            plugin_sha: String::new(),
            update: None,
            update_status: "No update check performed".to_owned(),
            driver: None,
            driver_status: "No driver metadata fetched".to_owned(),
            driver_sha: String::new(),
            driver_extract: String::new(),
        }
    }
}

fn dirs_home() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:\\Users\\Admin"))
}

impl HubApp {
    pub(super) fn lighting_ui(&mut self, ui: &mut egui::Ui) {
        theme::h2(ui, "Keyboard lighting");
        ui.label("Keymaps come from the installed renderer bundle; reports are sent through the live agent.");
        if ui.button("Load keymaps").clicked() {
            let asar = PathBuf::from("C:\\Program Files\\ATK HUB\\resources\\main\\app.asar");
            match atk_features::list_keymaps(&asar) {
                Ok(keymaps) => {
                    self.features.keymaps = keymaps.into_iter().map(|entry| entry.name).collect();
                    self.features.lighting_status =
                        format!("{} keymaps available", self.features.keymaps.len());
                }
                Err(error) => self.features.lighting_status = format!("Load failed: {error}"),
            }
        }
        ui.label(self.features.lighting_status.as_str());

        egui::ScrollArea::vertical()
            .max_height(180.0)
            .show(ui, |ui| {
                for name in &self.features.keymaps {
                    if ui
                        .selectable_label(name == &self.features.keymap, name)
                        .clicked()
                    {
                        self.features.keymap = name.clone();
                    }
                }
            });

        ui.horizontal(|ui| {
            ui.label("R");
            ui.text_edit_singleline(&mut self.features.red);
            ui.label("G");
            ui.text_edit_singleline(&mut self.features.green);
            ui.label("B");
            ui.text_edit_singleline(&mut self.features.blue);
            ui.label("Brightness");
            ui.text_edit_singleline(&mut self.features.brightness);
        });
        if theme::primary_button(ui, "Send lighting report").clicked() {
            let lighting = Lighting {
                keymap: self.features.keymap.clone(),
                red: self.features.red.parse().unwrap_or(0),
                green: self.features.green.parse().unwrap_or(0),
                blue: self.features.blue.parse().unwrap_or(0),
                brightness: self.features.brightness.parse().unwrap_or(0),
            };
            let report = lighting.report();
            self.run_control("lighting", |client| client.write_report(&report));
        }
    }

    pub(super) fn macros_ui(&mut self, ui: &mut egui::Ui) {
        theme::h2(ui, "Macros");
        ui.horizontal(|ui| {
            ui.label("Storage");
            ui.text_edit_singleline(&mut self.features.macro_path);
        });
        let load = ui.button("Load").clicked();
        let save = ui.button("Save").clicked();
        if load {
            let path = PathBuf::from(&self.features.macro_path);
            match atk_features::load_macros(&path) {
                Ok(macros) => {
                    self.features.macros = macros;
                    self.features.macro_status =
                        format!("{} macros loaded", self.features.macros.len());
                }
                Err(error) => self.features.macro_status = format!("Load failed: {error}"),
            }
        }
        if save {
            let path = PathBuf::from(&self.features.macro_path);
            match atk_features::save_macros(&path, &self.features.macros) {
                Ok(()) => self.features.macro_status = "Macros saved".to_owned(),
                Err(error) => self.features.macro_status = format!("Save failed: {error}"),
            }
        }

        ui.horizontal(|ui| {
            ui.label("Name");
            ui.text_edit_singleline(&mut self.features.macro_name);
            ui.label("Keys");
            ui.text_edit_singleline(&mut self.features.macro_keys);
        });
        if theme::primary_button(ui, "Add macro").clicked() {
            let steps: Vec<MacroStep> = self
                .features
                .macro_keys
                .split(',')
                .filter(|key| !key.trim().is_empty())
                .map(|key| MacroStep {
                    key: key.trim().to_owned(),
                    delay_ms: 50,
                })
                .collect();
            self.features.macros.push(Macro {
                name: self.features.macro_name.clone(),
                steps,
            });
            self.features.macro_status = format!("{} macros in memory", self.features.macros.len());
        }

        let mut run_index = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (index, macro_def) in self.features.macros.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "{}: {}",
                        macro_def.name,
                        macro_def
                            .steps
                            .iter()
                            .map(|step| step.key.as_str())
                            .collect::<Vec<_>>()
                            .join(" → ")
                    ));
                    if ui.button("Run").clicked() {
                        run_index = Some(index);
                    }
                });
            }
        });
        if let Some(index) = run_index {
            let steps = self.features.macros[index].steps.clone();
            let mut status = String::new();
            for step in steps {
                match atk_input::KeySequence::from_str(&step.key) {
                    Ok(sequence) => {
                        let _ = atk_input::KeyInjector.press(&sequence);
                    }
                    Err(error) => status = format!("Key error: {error}"),
                }
            }
            self.features.macro_status = if status.is_empty() {
                format!("Ran macro #{}", index + 1)
            } else {
                status
            };
        }
        ui.label(self.features.macro_status.as_str());
    }

    pub(super) fn music_ui(&mut self, ui: &mut egui::Ui) {
        theme::h2(ui, "Music rhythm");
        ui.label("The legacy rhythm service listens on loopback port 5577.");
        if theme::primary_button(ui, "Refresh status").clicked() {
            self.features.music_status = if atk_features::music_service_running() {
                "Service is running on 127.0.0.1:5577".to_owned()
            } else {
                "Service is not running".to_owned()
            };
        }
        if ui.button("Stop service").clicked() {
            self.features.music_status = match atk_features::stop_music_service() {
                Ok(()) => "Stop request sent".to_owned(),
                Err(error) => format!("Stop failed: {error}"),
            };
        }
        ui.label(self.features.music_status.as_str());
    }

    pub(super) fn pets_ui(&mut self, ui: &mut egui::Ui) {
        theme::h2(ui, "Desktop pets");
        ui.horizontal(|ui| {
            ui.label("Settings");
            ui.text_edit_singleline(&mut self.features.pet_path);
        });
        let load = ui.button("Load settings").clicked();
        let save = ui.button("Save settings").clicked();
        if load {
            let path = PathBuf::from(&self.features.pet_path);
            self.features.pets = match atk_features::load_pet_settings(&path) {
                Ok(settings) => {
                    self.features.pet_status = "Pet settings loaded".to_owned();
                    Some(settings)
                }
                Err(error) => {
                    self.features.pet_status = format!("Load failed: {error}");
                    None
                }
            };
        }
        if save {
            let text = self
                .features
                .pets
                .as_ref()
                .and_then(|settings| settings.to_json().ok());
            if let Some(text) = text {
                let path = PathBuf::from(&self.features.pet_path);
                self.features.pet_status = match std::fs::write(path, text) {
                    Ok(()) => "Pet settings saved".to_owned(),
                    Err(error) => format!("Save failed: {error}"),
                };
            }
        }
        if let Some(settings) = self.features.pets.as_mut() {
            for (label, key) in [
                ("Wooden fish", "woodenFish.enabled"),
                ("Hoops pet", "hoopsPet.enabled"),
            ] {
                let mut enabled = atk_features::pet_enabled(settings, key);
                if ui.checkbox(&mut enabled, label).clicked()
                    && let Err(error) = atk_features::set_pet_enabled(settings, key, enabled)
                {
                    self.features.pet_status = format!("Update failed: {error}");
                }
            }
        }
        ui.label(self.features.pet_status.as_str());
    }

    pub(super) fn plugins_ui(&mut self, ui: &mut egui::Ui) {
        theme::h2(ui, "Plugins");
        ui.horizontal(|ui| {
            ui.label("User data");
            ui.text_edit_singleline(&mut self.features.user_data);
        });
        if ui.button("Scan installed plugins").clicked() {
            let root = PathBuf::from(&self.features.user_data);
            match atk_features::list_plugins(&root) {
                Ok(plugins) => {
                    self.features.plugin_status = format!("{} plugins installed", plugins.len());
                    self.features.plugins = plugins;
                }
                Err(error) => self.features.plugin_status = format!("Scan failed: {error}"),
            }
        }
        ui.label(self.features.plugin_status.as_str());
        ui.separator();
        theme::h2(ui, "Install bundle");
        ui.horizontal(|ui| {
            ui.label("URL");
            ui.text_edit_singleline(&mut self.features.plugin_url);
        });
        ui.horizontal(|ui| {
            ui.label("Id");
            ui.text_edit_singleline(&mut self.features.plugin_id);
            ui.label("Version");
            ui.text_edit_singleline(&mut self.features.plugin_version);
        });
        ui.horizontal(|ui| {
            ui.label("SHA-256");
            ui.text_edit_singleline(&mut self.features.plugin_sha);
        });
        if theme::primary_button(ui, "Download, verify and install").clicked() {
            let root = PathBuf::from(&self.features.user_data);
            let url = self.features.plugin_url.clone();
            let id = self.features.plugin_id.clone();
            let version = self.features.plugin_version.clone();
            let sha = self.features.plugin_sha.clone();
            self.features.plugin_status =
                match atk_features::install_bundle(&url, &id, &version, &sha, &root) {
                    Ok(path) => format!("Installed {}", path.display()),
                    Err(error) => format!("Install failed: {error}"),
                };
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            for plugin in &self.features.plugins {
                ui.label(format!(
                    "{} v{} — {}",
                    plugin.id,
                    plugin.version,
                    plugin.path.display()
                ));
            }
        });
    }

    pub(super) fn updater_ui(&mut self, ui: &mut egui::Ui) {
        theme::h2(ui, "Updater");
        ui.label("Checks the production feed used by electron-updater.");
        if theme::primary_button(ui, "Check for updates").clicked() {
            match atk_features::check_update() {
                Ok(update) => {
                    self.features.update_status =
                        if atk_features::version_newer("3.2.31", &update.version) {
                            format!("Update available: {}", update.version)
                        } else {
                            format!("Up to date: {}", update.version)
                        };
                    self.features.update = Some(update);
                }
                Err(error) => self.features.update_status = format!("Check failed: {error}"),
            }
        }
        ui.label(self.features.update_status.as_str());
        if let Some(update) = &self.features.update {
            ui.label(format!("installer: {}", update.url));
            ui.label(format!("size: {} bytes", update.size));
            ui.label(format!("sha512: {}", update.sha512));
            if ui.button("Open installer URL").clicked() {
                let url = update.url.clone();
                self.run_control("open installer", |client| client.execute_action(&url));
            }
        }
    }

    pub(super) fn drivers_ui(&mut self, ui: &mut egui::Ui) {
        theme::h2(ui, "Driver downloads");
        ui.label("Fetches service metadata from api-hub-cloud.atk.pro.");
        if theme::primary_button(ui, "Fetch game-power service").clicked() {
            match atk_features::fetch_service_driver() {
                Ok(driver) => {
                    self.features.driver_status = format!("Remote version {}", driver.version);
                    self.features.driver = Some(driver);
                }
                Err(error) => self.features.driver_status = format!("Fetch failed: {error}"),
            }
        }
        ui.label(self.features.driver_status.as_str());
        ui.horizontal(|ui| {
            ui.label("Expected SHA-256");
            ui.text_edit_singleline(&mut self.features.driver_sha);
        });
        if ui.button("Download and extract to staging").clicked()
            && let Some(driver) = self.features.driver.clone()
        {
            let cache = std::env::temp_dir().join("atk-hub-driver-cache");
            let sha = self.features.driver_sha.clone();
            self.features.driver_status = match atk_features::download_driver(&driver, &cache, &sha)
            {
                Ok(path) => {
                    self.features.driver_extract = path.to_string_lossy().into_owned();
                    format!("Staged {}", path.display())
                }
                Err(error) => format!("Download failed: {error}"),
            };
        }
        if let Some(driver) = &self.features.driver {
            ui.label(format!("download: {}", driver.file));
            if ui.button("Open download URL").clicked() {
                let url = driver.file.clone();
                self.run_control("open download", |client| client.execute_action(&url));
            }
            if !self.features.driver_extract.is_empty() {
                ui.label(format!("staged: {}", self.features.driver_extract));
            }
        }
    }
}
