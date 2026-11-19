//! Graphical front end for the Rust port of ATK HUB.
//!
//! Every panel operates on the statically ported core crates: no JavaScript
//! runtime, Electron process or native ATK addon is loaded.

use atk_agent::{AgentClient, DeviceId, Frame, FramedBuffer, PipeName, Request, Rule, RuleAction};
use atk_asar::Manifest;
use atk_config::Config;
use atk_endpoints::{
    FirmwareService, device_update_version_url, firmware_service_url, firmware_url,
    music_service_exit_url, official_site_url, updater_feed_url, wallpaper_catalog_url,
};
use atk_ipc::{Category, all, by_category, is_agent_method};
use serde_json::Value;
use std::fs;

mod features_ui;
use features_ui::FeatureState;

/// Page shown in the central panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    /// Build identity and architecture summary.
    Overview,
    /// ASAR manifest inspector.
    Asar,
    /// `config.json` editor.
    Config,
    /// Remote endpoint reference.
    Endpoints,
    /// IPC channel and agent method registry.
    Ipc,
    /// Named-pipe frame playground.
    Agent,
    /// Live HID devices reported by the running agent.
    Devices,
    /// Live wallpaper catalogue from the ATK cloud.
    Wallpapers,
    /// THX audio effects over gRPC.
    Audio,
    /// Keyboard lighting.
    Lighting,
    /// Macro editor.
    Macros,
    /// Legacy music rhythm service.
    Music,
    /// Desktop pets.
    Pets,
    /// Installed plugins.
    Plugins,
    /// Updater status.
    Updater,
    /// Driver downloads.
    Drivers,
}

impl Tab {
    /// Every page, in navigation order.
    pub const ALL: [Self; 16] = [
        Self::Overview,
        Self::Asar,
        Self::Config,
        Self::Endpoints,
        Self::Ipc,
        Self::Agent,
        Self::Devices,
        Self::Wallpapers,
        Self::Audio,
        Self::Lighting,
        Self::Macros,
        Self::Music,
        Self::Pets,
        Self::Plugins,
        Self::Updater,
        Self::Drivers,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Asar => "ASAR",
            Self::Config => "Config",
            Self::Endpoints => "Endpoints",
            Self::Ipc => "IPC",
            Self::Agent => "Agent",
            Self::Devices => "Devices",
            Self::Wallpapers => "Wallpapers",
            Self::Audio => "Audio",
            Self::Lighting => "Lighting",
            Self::Macros => "Macros",
            Self::Music => "Music",
            Self::Pets => "Pets",
            Self::Plugins => "Plugins",
            Self::Updater => "Updater",
            Self::Drivers => "Drivers",
        }
    }
}

/// Action performed by the Audio page against the THX service.
#[derive(Debug, Clone, Copy)]
enum AudioAction {
    /// Read aggregated status.
    Status,
    /// Set the spatial audio toggle.
    SetSpatial(bool),
}

/// Application state for the ATK HUB Rust GUI.
#[derive(Debug)]
pub struct HubApp {
    tab: Tab,
    asar_path: String,
    asar_status: String,
    manifest: Option<Manifest>,
    asar_filter: String,
    config_path: String,
    config_status: String,
    config: Option<Config>,
    user_id: String,
    service_url: String,
    ui_version: String,
    config_json: String,
    vendor_id: String,
    product_id: String,
    tag: String,
    update_host: String,
    ipc_filter: String,
    selected_category: Option<Category>,
    pipe_name: String,
    pipe_status: String,
    request_id: String,
    request_method: String,
    request_params: String,
    encoded_request: String,
    frame_input: String,
    frame_output: String,
    client: Option<AgentClient>,
    live_status: String,
    live_result: String,
    devices: Vec<Value>,
    devices_status: String,
    catalog: Option<atk_wallpaper::Catalog>,
    wallpaper_status: String,
    wallpaper_filter: String,
    control_vid: String,
    control_pid: String,
    control_report: String,
    control_scope: String,
    control_rule_name: String,
    control_packet: String,
    control_target: String,
    control_status: String,
    agent_state: String,
    audio_product: String,
    audio_status: String,
    audio_details: String,
    features: FeatureState,
}

impl Default for HubApp {
    fn default() -> Self {
        Self::new()
    }
}

impl HubApp {
    /// Creates the application with empty inputs.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tab: Tab::Overview,
            asar_path: String::new(),
            asar_status: "No archive loaded".to_owned(),
            manifest: None,
            asar_filter: String::new(),
            config_path: String::new(),
            config_status: "No config loaded".to_owned(),
            config: None,
            user_id: String::new(),
            service_url: String::new(),
            ui_version: String::new(),
            config_json: String::new(),
            vendor_id: "0x1234".to_owned(),
            product_id: "0x5678".to_owned(),
            tag: "service".to_owned(),
            update_host: "bpcdn.atkgear.com".to_owned(),
            ipc_filter: String::new(),
            selected_category: None,
            pipe_name: "atk-hub-agent-session-1".to_owned(),
            pipe_status: String::new(),
            request_id: "1".to_owned(),
            request_method: "device.list".to_owned(),
            request_params: "null".to_owned(),
            encoded_request: String::new(),
            frame_input: String::new(),
            frame_output: String::new(),
            client: None,
            live_status: "Not connected".to_owned(),
            live_result: String::new(),
            devices: Vec::new(),
            devices_status: "No device list requested".to_owned(),
            catalog: None,
            wallpaper_status: "No catalogue loaded".to_owned(),
            wallpaper_filter: String::new(),
            control_vid: "0x3554".to_owned(),
            control_pid: "0xf505".to_owned(),
            control_report: "00 AA 01".to_owned(),
            control_scope: "rgb".to_owned(),
            control_rule_name: "launch".to_owned(),
            control_packet: "A1".to_owned(),
            control_target: "notepad.exe".to_owned(),
            control_status: "No control request sent".to_owned(),
            agent_state: String::new(),
            audio_product: atk_audio::DEFAULT_PRODUCT.to_owned(),
            audio_status: "No THX service queried".to_owned(),
            audio_details: String::new(),
            features: FeatureState::default(),
        }
    }

    /// Selects the page shown in the central panel.
    pub fn set_tab(&mut self, tab: Tab) {
        self.tab = tab;
    }

    /// Renders the whole interface into the given egui UI.
    pub fn render(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("navigation")
            .resizable(false)
            .exact_size(150.0)
            .show(ui, |ui| {
                ui.add_space(12.0);
                ui.heading("ATK HUB");
                ui.label("Rust port");
                ui.separator();
                for tab in Tab::ALL {
                    ui.selectable_value(&mut self.tab, tab, tab.label());
                }
                ui.add_space(8.0);
                ui.separator();
                ui.label("v3.2.31 behavior");
                ui.label("Static, no code execution");
            });

        egui::Panel::top("title").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading(self.tab.label());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label("atk-hub-rs");
                });
            });
        });

        let tab = self.tab;
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| match tab {
                Tab::Overview => self.overview_ui(ui),
                Tab::Asar => self.asar_ui(ui),
                Tab::Config => self.config_ui(ui),
                Tab::Endpoints => self.endpoints_ui(ui),
                Tab::Ipc => self.ipc_ui(ui),
                Tab::Agent => self.agent_ui(ui),
                Tab::Devices => self.devices_ui(ui),
                Tab::Wallpapers => self.wallpapers_ui(ui),
                Tab::Audio => self.audio_ui(ui),
                Tab::Lighting => self.lighting_ui(ui),
                Tab::Macros => self.macros_ui(ui),
                Tab::Music => self.music_ui(ui),
                Tab::Pets => self.pets_ui(ui),
                Tab::Plugins => self.plugins_ui(ui),
                Tab::Updater => self.updater_ui(ui),
                Tab::Drivers => self.drivers_ui(ui),
            });
        });
    }

    fn overview_ui(&self, ui: &mut egui::Ui) {
        ui.heading("Ported architecture");
        ui.add_space(6.0);
        ui.label("ATK HUB is three layers. This Rust application replaces the Electron main process and renderer with statically ported core crates and a native GUI.");
        ui.add_space(10.0);

        egui::Grid::new("architecture")
            .striped(true)
            .show(ui, |ui| {
                ui.heading("Layer");
                ui.heading("Rust equivalent");
                ui.end_row();
                for (original, port) in [
                    (
                        "atk-hub-service.exe (Rust, SYSTEM)",
                        "portable protocol + pipe naming in atk-agent",
                    ),
                    (
                        "Electron main / 169 IPC handlers",
                        "channel registry in atk-ipc",
                    ),
                    ("config.json (electron-store)", "typed access in atk-config"),
                    ("Update and firmware URLs", "templates in atk-endpoints"),
                    ("app.asar package manifest", "reader in atk-asar"),
                    ("Electron renderer", "this eframe/egui GUI"),
                ] {
                    ui.label(original);
                    ui.label(port);
                    ui.end_row();
                }
            });

        ui.add_space(12.0);
        ui.heading("Verification status");
        ui.label("cargo build, cargo test and cargo clippy run without warnings. GUI rendering is exercised by an offscreen egui test, so no window is opened by the test suite.");
    }

    fn asar_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("ASAR manifest inspector");
        ui.label("Path to an Electron app.asar (the installed ATK HUB archive is prefillable).");
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut self.asar_path);
            if ui.button("Load").clicked() {
                self.load_asar();
            }
        });
        ui.label(self.asar_status.as_str());
        ui.separator();

        if let Some(manifest) = &self.manifest {
            ui.horizontal(|ui| {
                ui.label(format!("{} files", manifest.file_count()));
                ui.label(format!("payload starts at 0x{:x}", manifest.data_offset));
            });
            ui.text_edit_singleline(&mut self.asar_filter);
            let filter = self.asar_filter.clone();
            egui::ScrollArea::vertical()
                .max_height(420.0)
                .show(ui, |ui| {
                    for path in manifest.paths() {
                        if !filter.is_empty() && !path.contains(&filter) {
                            continue;
                        }
                        let entry = manifest.lookup(&path).expect("path came from the manifest");
                        ui.horizontal(|ui| {
                            ui.label(path.as_str());
                            ui.label(format!("{} bytes", entry.size));
                            if entry.unpacked {
                                ui.label("unpacked");
                            }
                            if entry.executable {
                                ui.label("executable");
                            }
                        });
                    }
                });
        }
    }

    fn load_asar(&mut self) {
        match fs::read(&self.asar_path) {
            Ok(bytes) => match Manifest::parse(&bytes) {
                Ok(manifest) => {
                    self.asar_status = format!("Loaded {} top-level entries", manifest.files.len());
                    self.manifest = Some(manifest);
                }
                Err(error) => {
                    self.asar_status = format!("Parse failed: {error}");
                    self.manifest = None;
                }
            },
            Err(error) => {
                self.asar_status = format!("Read failed: {error}");
                self.manifest = None;
            }
        }
    }

    fn config_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("config.json editor");
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut self.config_path);
            if ui.button("Load").clicked() {
                self.load_config();
            }
        });
        ui.label(self.config_status.as_str());
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("User id");
            ui.text_edit_singleline(&mut self.user_id);
        });
        ui.horizontal(|ui| {
            ui.label("Service URL");
            ui.text_edit_singleline(&mut self.service_url);
        });
        ui.horizontal(|ui| {
            ui.label("UI version");
            ui.text_edit_singleline(&mut self.ui_version);
        });

        if ui.button("Apply to document").clicked() {
            self.apply_config();
        }
        ui.add_space(6.0);
        ui.add(egui::TextEdit::multiline(&mut self.config_json).desired_rows(12));
    }

    fn load_config(&mut self) {
        match fs::read_to_string(&self.config_path) {
            Ok(text) => match Config::parse(&text) {
                Ok(config) => {
                    self.user_id = config.user_id().unwrap_or_default().to_owned();
                    self.service_url = config.service_url().unwrap_or_default().to_owned();
                    self.ui_version = config
                        .acknowledged_ui_version()
                        .unwrap_or_default()
                        .to_owned();
                    self.config_json = text;
                    self.config_status = "Config loaded".to_owned();
                    self.config = Some(config);
                }
                Err(error) => self.config_status = format!("Parse failed: {error}"),
            },
            Err(error) => self.config_status = format!("Read failed: {error}"),
        }
    }

    fn apply_config(&mut self) {
        let Some(config) = self.config.as_mut() else {
            self.config_status = "Load a config first".to_owned();
            return;
        };
        if let Err(error) = config.set_service_url(&self.service_url) {
            self.config_status = format!("Update failed: {error}");
            return;
        }
        match config.to_json() {
            Ok(text) => {
                self.config_json = text;
                self.config_status = "Service URL updated (tab-indented JSON)".to_owned();
            }
            Err(error) => self.config_status = format!("Serialize failed: {error}"),
        }
    }

    fn endpoints_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Remote endpoints");
        ui.label("Identifiers are decimal in the generated URLs, matching JavaScript template interpolation.");
        ui.horizontal(|ui| {
            ui.label("Vendor");
            ui.text_edit_singleline(&mut self.vendor_id);
            ui.label("Product");
            ui.text_edit_singleline(&mut self.product_id);
            ui.label("Tag");
            ui.text_edit_singleline(&mut self.tag);
        });
        ui.horizontal(|ui| {
            ui.label("Update host");
            ui.text_edit_singleline(&mut self.update_host);
        });

        let vendor = parse_id(&self.vendor_id);
        let product = parse_id(&self.product_id);
        let urls = [
            ("BYKB service", firmware_service_url(FirmwareService::Bykb)),
            (
                "Game-power service",
                firmware_service_url(FirmwareService::GamePower),
            ),
            (
                "Firmware / model pack",
                firmware_url(vendor, product, &self.tag),
            ),
            ("Wallpaper catalogue", wallpaper_catalog_url().to_owned()),
            ("Updater feed", updater_feed_url().to_owned()),
            (
                "Version check",
                device_update_version_url(&self.update_host, "win32"),
            ),
            ("Official site", official_site_url().to_owned()),
            ("Music service exit", music_service_exit_url().to_owned()),
        ];

        ui.add_space(8.0);
        egui::Grid::new("endpoints").striped(true).show(ui, |ui| {
            for (name, url) in urls {
                ui.label(name);
                ui.label(url);
                ui.end_row();
            }
        });
    }

    fn ipc_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("IPC registry");
        ui.label(format!(
            "{} literal channels recovered from the main process",
            all().len()
        ));
        ui.text_edit_singleline(&mut self.ipc_filter);

        ui.horizontal(|ui| {
            if ui.button("All").clicked() {
                self.selected_category = None;
            }
            for (name, category) in [
                ("Device", Category::Device),
                ("Widget", Category::Widget),
                ("Automation", Category::Automation),
                ("Wallpaper", Category::Wallpaper),
                ("Game", Category::Game),
                ("Agent", Category::Agent),
                ("System", Category::System),
            ] {
                if ui.button(name).clicked() {
                    self.selected_category = Some(category);
                }
            }
        });

        let filter = self.ipc_filter.clone();
        let groups = by_category();
        egui::ScrollArea::vertical()
            .max_height(360.0)
            .show(ui, |ui| {
                for (group_category, channels) in &groups {
                    if self
                        .selected_category
                        .is_some_and(|selected| selected != *group_category)
                    {
                        continue;
                    }
                    ui.collapsing(format!("{group_category} ({})", channels.len()), |ui| {
                        for channel in channels {
                            if filter.is_empty() || channel.as_str().contains(&filter) {
                                ui.label((*channel).as_str());
                            }
                        }
                    });
                }
            });

        ui.separator();
        ui.heading("Rust-agent allowlist");
        ui.horizontal_wrapped(|ui| {
            for method in atk_ipc::AGENT_METHODS {
                let color = if is_agent_method(method) {
                    egui::Color32::LIGHT_GREEN
                } else {
                    egui::Color32::RED
                };
                ui.colored_label(color, *method);
            }
        });
        ui.label("Dynamic handler names in the original cannot be recovered statically.");
    }

    fn connect_live(&mut self) {
        let name = match PipeName::parse(&self.pipe_name) {
            Ok(name) => name,
            Err(error) => {
                self.live_status = format!("Invalid pipe: {error}");
                return;
            }
        };
        match AgentClient::connect(&name) {
            Ok(client) => {
                self.live_status = format!("Connected to {}", name.windows_path());
                self.client = Some(client);
            }
            Err(error) => {
                self.live_status = format!("Connect failed: {error}");
                self.client = None;
            }
        }
    }

    fn send_live_request(&mut self) {
        let Some(client) = self.client.as_ref() else {
            self.live_status = "Connect to the agent first".to_owned();
            return;
        };
        let params = serde_json::from_str(&self.request_params).unwrap_or(Value::Null);
        match client.request(&self.request_method, params) {
            Ok(value) => {
                self.live_result =
                    serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
                self.live_status = format!("{} succeeded", self.request_method);
            }
            Err(error) => {
                self.live_result.clear();
                self.live_status = format!("Request failed: {error}");
            }
        }
    }

    fn refresh_devices(&mut self) {
        let Some(client) = self.client.as_ref() else {
            self.devices_status = "Connect to the agent on the Agent page first".to_owned();
            return;
        };
        match client.request("device.list", Value::Null) {
            Ok(Value::Array(items)) => {
                self.devices_status = format!("{} live HID interfaces", items.len());
                self.devices = items;
            }
            Ok(other) => self.devices_status = format!("Unexpected response: {other}"),
            Err(error) => self.devices_status = format!("Request failed: {error}"),
        }
    }

    fn audio_config(&mut self) {
        match atk_audio::load_endpoint_config(&self.audio_product) {
            Ok(config) => {
                self.audio_status = format!(
                    "Connection file valid: {} (TLS certificate present)",
                    config.address
                );
            }
            Err(error) => self.audio_status = format!("Connection file error: {error}"),
        }
    }

    fn run_audio(&mut self, action: AudioAction) {
        let product = self.audio_product.clone();
        let config = match atk_audio::load_endpoint_config(&product) {
            Ok(config) => config,
            Err(error) => {
                self.audio_status = format!("Connection file error: {error}");
                return;
            }
        };
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                self.audio_status = format!("Runtime error: {error}");
                return;
            }
        };
        let result = runtime.block_on(async {
            let mut client = atk_audio::ThxClient::connect(&config).await?;
            match action {
                AudioAction::Status => {
                    let status = client.status().await?;
                    serde_json::to_string_pretty(&status).map_err(atk_audio::AudioError::from)
                }
                AudioAction::SetSpatial(value) => {
                    client
                        .set_bool(
                            atk_audio::HEADPHONES_ENDPOINT,
                            atk_audio::SPATIAL_ENABLED,
                            value,
                        )
                        .await?;
                    Ok("Spatial setting updated".to_owned())
                }
            }
        });
        match result {
            Ok(text) => {
                self.audio_details = text;
                self.audio_status = "THX request succeeded".to_owned();
            }
            Err(error) => self.audio_status = format!("THX request failed: {error}"),
        }
    }

    fn audio_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("THX audio (gRPC over TLS)");
        ui.label(
            "Reads C:\\ProgramData\\thx\\<product>\\thx_properties.tcp, like the original client.",
        );
        ui.horizontal(|ui| {
            ui.label("Product");
            ui.text_edit_singleline(&mut self.audio_product);
        });
        if ui.button("Validate connection file").clicked() {
            self.audio_config();
        }
        ui.label(self.audio_status.as_str());
        ui.separator();

        let read = ui.button("Read status").clicked();
        let enable = ui.button("Enable spatial").clicked();
        let disable = ui.button("Disable spatial").clicked();
        if read {
            self.run_audio(AudioAction::Status);
        }
        if enable {
            self.run_audio(AudioAction::SetSpatial(true));
        }
        if disable {
            self.run_audio(AudioAction::SetSpatial(false));
        }
        ui.add(egui::TextEdit::multiline(&mut self.audio_details).desired_rows(14));
    }

    fn refresh_catalog(&mut self) {
        self.wallpaper_status = "Downloading catalogue…".to_owned();
        match atk_wallpaper::fetch_catalog() {
            Ok(catalog) => {
                self.wallpaper_status = format!("{} wallpapers available", catalog.wallpaper.len());
                self.catalog = Some(catalog);
            }
            Err(error) => {
                self.wallpaper_status = format!("Download failed: {error}");
                self.catalog = None;
            }
        }
    }

    fn wallpapers_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Wallpaper catalogue");
        ui.label("Live entries from wallpaper.atkgear.com/list-v2.json.");
        if ui.button("Refresh catalogue").clicked() {
            self.refresh_catalog();
        }
        ui.label(self.wallpaper_status.as_str());
        ui.text_edit_singleline(&mut self.wallpaper_filter);
        ui.separator();

        let entries: Vec<_> = self
            .catalog
            .as_ref()
            .map(|catalog| catalog.search(&self.wallpaper_filter))
            .unwrap_or_default();
        egui::ScrollArea::vertical().show(ui, |ui| {
            for entry in entries {
                ui.collapsing(format!("#{} {}", entry.id, entry.title), |ui| {
                    ui.label(format!("asset: {}", entry.value));
                    ui.label(format!("version: {}", entry.version));
                    ui.label(format!("preview: {}", entry.preview_url));
                    ui.label(format!("download: {}", entry.download_url));
                });
            }
        });
    }

    fn control_device(&self) -> DeviceId {
        DeviceId::new(parse_id(&self.control_vid), parse_id(&self.control_pid))
    }

    fn run_control(
        &mut self,
        label: &str,
        operation: impl FnOnce(&AgentClient) -> Result<Value, atk_agent::AgentError>,
    ) {
        let Some(client) = self.client.as_ref() else {
            self.control_status = "Connect to the agent on the Agent page first".to_owned();
            return;
        };
        self.control_status = match operation(client) {
            Ok(value) => format!("{label}: {value}"),
            Err(error) => format!("{label} failed: {error}"),
        };
    }

    fn refresh_state(&mut self) {
        let Some(client) = self.client.as_ref() else {
            self.control_status = "Connect to the agent on the Agent page first".to_owned();
            return;
        };
        match client.state() {
            Ok(state) => {
                self.agent_state =
                    serde_json::to_string_pretty(&state).unwrap_or_else(|_| format!("{state:?}"));
                self.control_status = "state.get succeeded".to_owned();
            }
            Err(error) => self.control_status = format!("state.get failed: {error}"),
        }
    }

    fn parse_report(text: &str) -> Option<Vec<u8>> {
        text.split(|character: char| character.is_ascii_whitespace() || character == ',')
            .filter(|part| !part.is_empty())
            .map(|part| u8::from_str_radix(part.strip_prefix("0x").unwrap_or(part), 16).ok())
            .collect()
    }

    fn devices_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Live HID devices");
        ui.label("Connect to the agent on the Agent page, then refresh this list.");
        if ui.button("Refresh devices").clicked() {
            self.refresh_devices();
        }
        ui.label(self.devices_status.as_str());
        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| {
            for device in &self.devices {
                let product = device
                    .get("product")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                let vid = device.get("vid").and_then(Value::as_u64).unwrap_or(0);
                let pid = device.get("pid").and_then(Value::as_u64).unwrap_or(0);
                let usage = device.get("usage").and_then(Value::as_u64).unwrap_or(0);
                ui.label(format!(
                    "{product} — VID {vid:#06x} PID {pid:#06x} usage {usage}"
                ));
            }
        });

        ui.separator();
        ui.heading("HID control");
        ui.horizontal(|ui| {
            ui.label("VID");
            ui.text_edit_singleline(&mut self.control_vid);
            ui.label("PID");
            ui.text_edit_singleline(&mut self.control_pid);
        });
        let connect = ui.button("Connect device").clicked();
        let disconnect = ui.button("Disconnect").clicked();
        let state = ui.button("Refresh state").clicked();
        if connect {
            let device = self.control_device();
            self.run_control("device.connect", |client| client.connect_device(&device));
        }
        if disconnect {
            self.run_control("device.disconnect", |client| client.disconnect_device());
        }
        if state {
            self.refresh_state();
        }

        ui.horizontal(|ui| {
            ui.label("Report bytes");
            ui.text_edit_singleline(&mut self.control_report);
        });
        if ui.button("Write report").clicked() {
            match Self::parse_report(&self.control_report) {
                Some(report) => {
                    self.run_control("device.write", |client| client.write_report(&report))
                }
                None => {
                    self.control_status =
                        "Report must be space/comma separated hex bytes".to_owned()
                }
            }
        }
        if ui.button("Clear rules").clicked() {
            self.run_control("rules.set", |client| client.set_rules(&[]));
        }

        ui.horizontal(|ui| {
            ui.label("Scope");
            ui.text_edit_singleline(&mut self.control_scope);
            ui.label("Rule");
            ui.text_edit_singleline(&mut self.control_rule_name);
        });
        ui.horizontal(|ui| {
            ui.label("Packet");
            ui.text_edit_singleline(&mut self.control_packet);
            ui.label("Target");
            ui.text_edit_singleline(&mut self.control_target);
        });
        if ui.button("Set bindings").clicked() {
            let device = self.control_device();
            let scope = self.control_scope.clone();
            let rule = Rule::new(
                self.control_rule_name.clone(),
                self.control_packet.clone(),
                RuleAction::open(self.control_target.clone()),
            );
            self.run_control("bindings.set", |client| {
                client.set_bindings(&device, &scope, &[rule])
            });
        }

        ui.label(self.control_status.as_str());
        ui.add(egui::TextEdit::multiline(&mut self.agent_state).desired_rows(5));
    }

    fn agent_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Named-pipe protocol");
        ui.horizontal(|ui| {
            ui.label("Pipe");
            ui.text_edit_singleline(&mut self.pipe_name);
            if ui.button("Validate").clicked() {
                self.pipe_status = match PipeName::parse(&self.pipe_name) {
                    Ok(name) => format!("Valid: {}", name.windows_path()),
                    Err(error) => format!("Invalid: {error}"),
                };
            }
        });
        ui.label(self.pipe_status.as_str());
        ui.separator();

        ui.heading("Live connection");
        ui.horizontal(|ui| {
            if ui.button("Connect").clicked() {
                self.connect_live();
            }
            if ui.button("Disconnect").clicked() {
                self.client = None;
                self.live_status = "Disconnected".to_owned();
            }
            if ui.button("Send request").clicked() {
                self.send_live_request();
            }
        });
        ui.label(self.live_status.as_str());
        ui.add(egui::TextEdit::multiline(&mut self.live_result).desired_rows(4));
        ui.separator();

        ui.heading("Request frame");
        ui.horizontal(|ui| {
            ui.label("Id");
            ui.text_edit_singleline(&mut self.request_id);
            ui.label("Method");
            ui.text_edit_singleline(&mut self.request_method);
        });
        ui.horizontal(|ui| {
            ui.label("Params");
            ui.text_edit_singleline(&mut self.request_params);
            if ui.button("Encode").clicked() {
                self.encode_request();
            }
        });
        ui.add(egui::TextEdit::multiline(&mut self.encoded_request).desired_rows(3));

        ui.separator();
        ui.heading("Response decoder");
        ui.add(egui::TextEdit::multiline(&mut self.frame_input).desired_rows(3));
        if ui.button("Decode").clicked() {
            self.decode_frame();
        }
        ui.add(egui::TextEdit::multiline(&mut self.frame_output).desired_rows(5));
    }

    fn encode_request(&mut self) {
        let id = self.request_id.parse::<u64>().unwrap_or(0);
        let params = serde_json::from_str::<Value>(&self.request_params).unwrap_or(Value::Null);
        let request = Request::new(id, self.request_method.clone(), params);
        self.encoded_request = match request.encode() {
            Ok(bytes) => String::from_utf8_lossy(&bytes).replace('\n', "\\n"),
            Err(error) => format!("Error: {error}"),
        };
    }

    fn decode_frame(&mut self) {
        let mut buffer = FramedBuffer::default();
        let mut chunk = self.frame_input.clone();
        chunk.push('\n');
        self.frame_output = match buffer.push(chunk.as_bytes()) {
            Ok(frames) => frames
                .iter()
                .map(|frame| match frame {
                    Frame::Response {
                        id,
                        ok,
                        result,
                        error,
                    } => format!("response id={id} ok={ok} result={result} error={error:?}"),
                    Frame::Event { event, payload } => format!("event {event}: {payload}"),
                })
                .collect::<Vec<_>>()
                .join("\n"),
            Err(error) => format!("Error: {error}"),
        };
    }
}

fn parse_id(text: &str) -> u16 {
    let trimmed = text.trim();
    let without_prefix = trimmed.strip_prefix("0x").unwrap_or(trimmed);
    u16::from_str_radix(
        without_prefix,
        if trimmed.starts_with("0x") { 16 } else { 10 },
    )
    .unwrap_or(0)
}

impl eframe::App for HubApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.render(ui);
    }
}
