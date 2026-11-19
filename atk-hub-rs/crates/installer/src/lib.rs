//! License-free Windows installer for the ATK HUB Rust port.
//!
//! Uses only the Rust standard library and built-in Windows tooling. It copies
//! the portable package next to this executable into a per-user directory and
//! optionally creates a Start Menu shortcut. No third-party installer runtime
//! or license key is involved.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;

/// Main GUI executable shipped in the portable package.
const APP_EXE: &str = "atk-hub-rs.exe";
/// Headless CLI shipped in the portable package.
const CLI_EXE: &str = "atk-hub-cli.exe";
/// Start Menu shortcut name.
const SHORTCUT_NAME: &str = "ATK HUB (Rust).lnk";
/// Hide the helper PowerShell window entirely.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// Portable package used when setup.exe is run by itself.
const RELEASE_ZIP_URL: &str =
    "https://github.com/khentmoba/atk-hub-rs/releases/download/v0.1.3/atk-hub-rs-portable.zip";
/// Log written beside the system temp directory.
const LOG_NAME: &str = "atk-hub-rs-setup.log";
/// Logo shown by the installer and shipped in the portable package.
pub const LOGO_NAME: &str = "atk-hub-logo.png";

/// Failures during install or uninstall.
#[derive(Debug, Error)]
pub enum InstallError {
    /// File I/O failed.
    #[error("installer I/O failed")]
    Io(#[from] std::io::Error),
    /// A required package file was missing.
    #[error("portable package is missing {path}")]
    MissingFile {
        /// Missing file path.
        path: PathBuf,
    },
    /// Shortcut creation failed.
    #[error("shortcut creation failed: {detail}")]
    Shortcut {
        /// Diagnostic detail.
        detail: String,
    },
    /// The standalone installer could not download the portable package.
    #[error("bootstrap failed: {detail}")]
    Bootstrap {
        /// Diagnostic detail.
        detail: String,
    },
}

/// Paths controlling an installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallOptions {
    /// Directory containing the portable package files.
    pub source: PathBuf,
    /// Destination directory.
    pub target: PathBuf,
    /// Whether to create a Start Menu shortcut.
    pub shortcut: bool,
}

/// Result of an installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallReport {
    /// Installed application path.
    pub app: PathBuf,
    /// Installed CLI path.
    pub cli: PathBuf,
    /// Uninstaller script path.
    pub uninstaller: PathBuf,
}

fn quote(path: &Path) -> String {
    path.to_string_lossy().replace('\'', "''")
}

fn uninstall_script(target: &Path) -> String {
    format!(
        "$target = '{0}'\n\
         $shortcut = Join-Path ([Environment]::GetFolderPath('StartMenu')) 'Programs\\{1}'\n\
         if (Test-Path -LiteralPath $shortcut) {{ Remove-Item -LiteralPath $shortcut -Force }}\n\
         if (Test-Path -LiteralPath $target) {{ Remove-Item -LiteralPath $target -Recurse -Force }}\n\
         Write-Host 'ATK HUB Rust port uninstalled'\n",
        quote(target),
        SHORTCUT_NAME
    )
}

fn create_shortcut(target: &Path) -> Result<PathBuf, InstallError> {
    let start_menu = std::env::var("APPDATA")
        .map(|value| {
            PathBuf::from(value)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
        })
        .unwrap_or_else(|_| PathBuf::from("C:\\ProgramData\\Microsoft\\Windows\\Start Menu"));
    let programs = start_menu.join("Programs");
    fs::create_dir_all(&programs)?;
    let shortcut = programs.join(SHORTCUT_NAME);
    let app = target.join(APP_EXE);
    let script = format!(
        "$w = New-Object -ComObject WScript.Shell; $s = $w.CreateShortcut('{0}'); $s.TargetPath = '{1}'; $s.WorkingDirectory = '{2}'; $s.IconLocation = '{1},0'; $s.Save()",
        quote(&shortcut),
        quote(&app),
        quote(target)
    );
    let status = {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            let mut command = Command::new("powershell.exe");
            command
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-WindowStyle",
                    "Hidden",
                    "-Command",
                    &script,
                ])
                .creation_flags(CREATE_NO_WINDOW);
            command.status()?
        }
        #[cfg(not(windows))]
        {
            let _ = &script;
            return Err(InstallError::Shortcut {
                detail: "Windows only".to_owned(),
            });
        }
    };
    if !status.success() {
        return Err(InstallError::Shortcut {
            detail: format!("exit status {status}"),
        });
    }
    Ok(shortcut)
}

fn hidden_powershell(script: &str) -> Result<bool, InstallError> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let status = Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-WindowStyle",
                "Hidden",
                "-Command",
                script,
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .status()?;
        Ok(status.success())
    }
    #[cfg(not(windows))]
    {
        let _ = script;
        Ok(false)
    }
}

/// Appends a line to the installer log in the system temp directory.
pub fn log_line(line: &str) {
    use std::io::Write;
    let path = std::env::temp_dir().join(LOG_NAME);
    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{line}");
    }
}

/// Shows an error dialog when setup.exe is launched without a console.
pub fn show_error(message: &str) {
    let escaped = message.replace('\'', "''");
    let script = format!(
        "Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.MessageBox]::Show('{escaped}', 'ATK HUB installer', 'OK', 'Error')"
    );
    let _ = hidden_powershell(&script);
}

/// Shows the ATK HUB logo while setup runs.
pub fn show_logo(image: &Path) {
    let path = image.to_string_lossy().replace('\'', "''");
    let script = format!(
        "Add-Type -AssemblyName System.Windows.Forms; Add-Type -AssemblyName System.Drawing; $image = [System.Drawing.Image]::FromFile('{path}'); $form = New-Object System.Windows.Forms.Form; $form.Text = 'ATK HUB'; $form.Width = 340; $form.Height = 380; $form.StartPosition = 'CenterScreen'; $form.FormBorderStyle = 'FixedDialog'; $picture = New-Object System.Windows.Forms.PictureBox; $picture.Image = $image; $picture.Dock = 'Fill'; $picture.SizeMode = 'Zoom'; $form.Controls.Add($picture); $form.Show(); Start-Sleep -Milliseconds 1200; $form.Close(); $image.Dispose()"
    );
    let _ = hidden_powershell(&script);
}

/// Returns a directory containing the portable package.
///
/// When `setup.exe` is run alone, it downloads and extracts the release ZIP
/// instead of silently failing on missing sibling binaries.
///
/// # Errors
///
/// Returns an error if the download or extraction fails.
pub fn bootstrap(source: &Path) -> Result<PathBuf, InstallError> {
    if source.join(APP_EXE).is_file() && source.join(CLI_EXE).is_file() {
        return Ok(source.to_path_buf());
    }

    log_line("package files missing beside setup.exe, downloading portable zip");
    let temporary = std::env::temp_dir().join("atk-hub-rs-bootstrap");
    let _ = fs::remove_dir_all(&temporary);
    fs::create_dir_all(&temporary)?;
    let archive = temporary.join("package.zip");
    let download = format!(
        "Invoke-WebRequest -Uri '{RELEASE_ZIP_URL}' -OutFile '{}' -UseBasicParsing",
        quote(&archive)
    );
    if !hidden_powershell(&download)? {
        return Err(InstallError::Bootstrap {
            detail: "download failed".to_owned(),
        });
    }
    let extract = format!(
        "Expand-Archive -LiteralPath '{}' -DestinationPath '{}' -Force",
        quote(&archive),
        quote(&temporary)
    );
    if !hidden_powershell(&extract)? {
        return Err(InstallError::Bootstrap {
            detail: "extraction failed".to_owned(),
        });
    }
    log_line("portable zip extracted");
    Ok(temporary)
}

/// Installs the portable package into `target`.
///
/// # Errors
///
/// Returns an error for missing package files, filesystem failures, or a
/// failed shortcut creation.
pub fn install(options: &InstallOptions) -> Result<InstallReport, InstallError> {
    fs::create_dir_all(&options.target)?;
    for name in [APP_EXE, CLI_EXE] {
        let source = options.source.join(name);
        if !source.is_file() {
            return Err(InstallError::MissingFile { path: source });
        }
        fs::copy(&source, options.target.join(name))?;
    }
    let uninstaller = options.target.join("uninstall.ps1");
    fs::write(&uninstaller, uninstall_script(&options.target))?;
    fs::write(
        options.target.join("README.txt"),
        "ATK HUB Rust port\n\nRun atk-hub-rs.exe to launch the GUI.\n\
         Run atk-hub-cli.exe --help for headless tools.\n\
         Run powershell -File uninstall.ps1 to remove this installation.\n",
    )?;
    if options.shortcut {
        create_shortcut(&options.target)?;
    }
    Ok(InstallReport {
        app: options.target.join(APP_EXE),
        cli: options.target.join(CLI_EXE),
        uninstaller,
    })
}

/// Removes an installed copy and its shortcut.
///
/// # Errors
///
/// Returns an error if filesystem removal fails.
pub fn uninstall(target: &Path) -> Result<(), InstallError> {
    let start_menu = std::env::var("APPDATA")
        .map(|value| {
            PathBuf::from(value)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
        })
        .unwrap_or_else(|_| PathBuf::from("C:\\ProgramData\\Microsoft\\Windows\\Start Menu"));
    let shortcut = start_menu.join("Programs").join(SHORTCUT_NAME);
    if shortcut.exists() {
        fs::remove_file(shortcut)?;
    }
    if target.exists() {
        fs::remove_dir_all(target)?;
    }
    Ok(())
}

/// Returns the default per-user installation directory.
///
/// # Errors
///
/// Returns an error if `LOCALAPPDATA` is not set.
pub fn default_target() -> Result<PathBuf, InstallError> {
    std::env::var("LOCALAPPDATA")
        .map(|value| PathBuf::from(value).join("AtkVhub"))
        .map_err(|_| InstallError::Shortcut {
            detail: "LOCALAPPDATA is not set".to_owned(),
        })
}
