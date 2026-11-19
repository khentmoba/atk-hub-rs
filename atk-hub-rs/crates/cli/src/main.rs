//! Headless CLI for inspecting the ported ATK HUB behavior.

use atk_agent::{AgentClient, Frame, FramedBuffer, PipeName, Request};
use atk_asar::Manifest;
use atk_config::Config;
use atk_endpoints::{FirmwareService, firmware_service_url, firmware_url};
use atk_ipc::{all, by_category};
use clap::{Parser, Subcommand};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::Command as ProcessCommand;

#[derive(Debug, Parser)]
#[command(name = "atk-hub-cli", about = "Rust port of the ATK HUB core")]
struct Cli {
    /// Operation to perform.
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print the recovered remote URL templates.
    Endpoints,
    /// List recovered IPC channels grouped by feature area.
    Ipc,
    /// Download and print the live wallpaper catalogue.
    Wallpapers,
    /// Print allowlisted Rust-agent methods.
    AgentMethods,
    /// Encode an agent request frame.
    Encode {
        /// Monotonically increasing request id.
        id: u64,
        /// Agent method name.
        method: String,
        /// JSON parameters.
        #[arg(default_value = "null")]
        params: String,
    },
    /// Manage the installed ATK HUB Windows service.
    Service {
        /// Service action.
        #[arg(value_enum)]
        action: ServiceAction,
        /// Application path for `install`.
        #[arg(long)]
        app: Option<PathBuf>,
    },
    /// Ask the installed service for its session pipe name.
    Discover,
    /// Check the production update feed.
    Update,
    /// Fetch game-power driver metadata.
    Driver,
    /// Query the THX audio service.
    Audio {
        /// THX product id.
        #[arg(long, default_value = atk_audio::DEFAULT_PRODUCT)]
        product: String,
        /// Audio action.
        #[arg(value_enum)]
        action: AudioAction,
    },
    /// Read typed agent state through the live control API.
    DeviceState,
    /// Send a live request to the running agent.
    Live {
        /// Allowlisted agent method.
        method: String,
        /// JSON parameters.
        #[arg(default_value = "null")]
        params: String,
    },
    /// Decode a newline-delimited agent response frame.
    Decode {
        /// JSON frame text.
        frame: String,
    },
    /// Inspect an Electron ASAR manifest.
    Asar {
        /// Path to an app.asar file.
        path: PathBuf,
    },
    /// Show typed values from a config.json file.
    Config {
        /// Path to a config.json file.
        path: PathBuf,
    },
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum ServiceAction {
    /// Install the service with `--app <path>`.
    Install,
    /// Remove the service.
    Uninstall,
    /// Start the service.
    Start,
    /// Stop the service.
    Stop,
    /// Report service state.
    Status,
    /// Enable launch at boot.
    AutoLaunchEnable,
    /// Disable launch at boot.
    AutoLaunchDisable,
    /// Report launch-at-boot state.
    AutoLaunchStatus,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum AudioAction {
    /// Validate and print the connection file.
    Config,
    /// Read aggregated THX status.
    Status,
    /// Enable spatial audio.
    EnableSpatial,
    /// Disable spatial audio.
    DisableSpatial,
}

const SERVICE_EXE: &str = r"C:\Program Files\ATK HUB\resources\service\atk-hub-service.exe";

fn run_service(arguments: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let output = ProcessCommand::new(SERVICE_EXE).args(arguments).output()?;
    print!("{}", String::from_utf8_lossy(&output.stdout));
    if !output.status.success() {
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
        return Err(format!("service command exited with {output:?}").into());
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        Command::Endpoints => {
            println!(
                "bykb service: {}",
                firmware_service_url(FirmwareService::Bykb)
            );
            println!(
                "game service: {}",
                firmware_service_url(FirmwareService::GamePower)
            );
            println!("firmware: {}", firmware_url(0x1234, 0x5678, "service"));
        }
        Command::Ipc => {
            for (category, channels) in by_category() {
                println!("[{category}] {}", channels.len());
                for channel in channels {
                    println!("  {channel}");
                }
            }
            println!("total: {}", all().len());
        }
        Command::Wallpapers => {
            let catalog = atk_wallpaper::fetch_catalog()?;
            for entry in &catalog.wallpaper {
                println!(
                    "#{} {} ({}) -> {}",
                    entry.id, entry.title, entry.value, entry.download_url
                );
            }
            println!("total: {}", catalog.wallpaper.len());
        }
        Command::AgentMethods => {
            for method in atk_ipc::AGENT_METHODS {
                println!("{method}");
            }
        }
        Command::Encode { id, method, params } => {
            let params: Value = serde_json::from_str(&params)?;
            let request = Request::new(id, method, params);
            let bytes = request.encode()?;
            println!("{}", String::from_utf8_lossy(&bytes).replace('\n', "\\n"));
        }
        Command::Service { action, app } => match action {
            ServiceAction::Install => {
                let path = app.ok_or("--app is required for install")?;
                let path = path.to_string_lossy().to_string();
                run_service(&["install", "--app", &path])?;
            }
            ServiceAction::Uninstall => run_service(&["uninstall"])?,
            ServiceAction::Start => run_service(&["start"])?,
            ServiceAction::Stop => run_service(&["stop"])?,
            ServiceAction::Status => run_service(&["status"])?,
            ServiceAction::AutoLaunchEnable => run_service(&["auto-launch", "enable"])?,
            ServiceAction::AutoLaunchDisable => run_service(&["auto-launch", "disable"])?,
            ServiceAction::AutoLaunchStatus => run_service(&["auto-launch", "status"])?,
        },
        Command::Discover => {
            let output = ProcessCommand::new(
                r"C:\Program Files\ATK HUB\resources\service\atk-hub-service.exe",
            )
            .arg("agent-pipe")
            .output()?;
            let name = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            let pipe = PipeName::parse(&name)?;
            println!("{}", pipe.windows_path());
        }
        Command::Update => {
            let update = atk_features::check_update()?;
            println!("version: {}", update.version);
            println!(
                "newer than 3.2.31: {}",
                atk_features::version_newer("3.2.31", &update.version)
            );
            println!("installer: {}", update.url);
            println!("size: {}", update.size);
        }
        Command::Driver => {
            let driver = atk_features::fetch_service_driver()?;
            println!("version: {}", driver.version);
            println!("file: {}", driver.file);
        }
        Command::Audio { product, action } => {
            let config = atk_audio::load_endpoint_config(&product)?;
            println!("address: {}", config.address);
            if matches!(action, AudioAction::Config) {
                return Ok(());
            }
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            let result = runtime.block_on(async {
                let mut client = atk_audio::ThxClient::connect(&config).await?;
                match action {
                    AudioAction::Config => Ok(String::new()),
                    AudioAction::Status => {
                        let status = client.status().await?;
                        serde_json::to_string_pretty(&status).map_err(atk_audio::AudioError::from)
                    }
                    AudioAction::EnableSpatial | AudioAction::DisableSpatial => {
                        let enabled = matches!(action, AudioAction::EnableSpatial);
                        client
                            .set_bool(
                                atk_audio::HEADPHONES_ENDPOINT,
                                atk_audio::SPATIAL_ENABLED,
                                enabled,
                            )
                            .await?;
                        Ok(format!("spatial={enabled}"))
                    }
                }
            });
            println!("{}", result?);
        }
        Command::DeviceState => {
            let output = ProcessCommand::new(SERVICE_EXE)
                .arg("agent-pipe")
                .output()?;
            let name = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            let pipe = PipeName::parse(&name)?;
            let client = AgentClient::connect(&pipe)?;
            let state = client.state()?;
            println!("{state:?}");
        }
        Command::Live { method, params } => {
            let output = ProcessCommand::new(
                r"C:\Program Files\ATK HUB\resources\service\atk-hub-service.exe",
            )
            .arg("agent-pipe")
            .output()?;
            let name = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            let pipe = PipeName::parse(&name)?;
            let client = AgentClient::connect(&pipe)?;
            let parameters: Value = serde_json::from_str(&params)?;
            let events = client.events();
            let result = client.request(&method, parameters)?;
            println!("result: {result}");
            match events.try_recv() {
                Ok(event) => println!("event: {event}"),
                Err(_) => println!("events: none pending"),
            }
        }
        Command::Decode { frame } => {
            let mut input = frame;
            input.push('\n');
            let mut buffer = FramedBuffer::default();
            for decoded in buffer.push(input.as_bytes())? {
                match decoded {
                    Frame::Response {
                        id,
                        ok,
                        result,
                        error,
                    } => {
                        println!("response id={id} ok={ok} result={result} error={error:?}");
                    }
                    Frame::Event { event, payload } => println!("event {event}: {payload}"),
                }
            }
        }
        Command::Asar { path } => {
            let manifest = Manifest::parse(&fs::read(path)?)?;
            println!("files: {}", manifest.file_count());
            println!("data offset: 0x{:x}", manifest.data_offset);
            for entry in manifest.paths() {
                println!("{entry}");
            }
        }
        Command::Config { path } => {
            let config = Config::parse(&fs::read_to_string(path)?)?;
            println!("user id: {:?}", config.user_id());
            println!("service url: {:?}", config.service_url());
            println!("ui version: {:?}", config.acknowledged_ui_version());
        }
    }
    Ok(())
}
