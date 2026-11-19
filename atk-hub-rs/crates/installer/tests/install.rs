//! Installer tests that never touch real user directories or shortcuts.

use atk_installer::{InstallError, InstallOptions, bootstrap, install, uninstall};
use std::fs;

struct Fixture {
    root: std::path::PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("atk-installer-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("package")).expect("package dir");
        for file in ["atk-hub-rs.exe", "atk-hub-cli.exe"] {
            fs::write(root.join("package").join(file), format!("fake {file}"))
                .expect("write package");
        }
        Self { root }
    }

    fn options(&self) -> InstallOptions {
        InstallOptions {
            source: self.root.join("package"),
            target: self.root.join("installed"),
            shortcut: false,
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn bootstrap_uses_an_existing_portable_package_without_network() {
    let fixture = Fixture::new("bootstrap");
    let options = fixture.options();
    let source = bootstrap(&options.source).expect("existing package");
    assert_eq!(source, options.source);
}

#[test]
fn installs_portable_files_and_uninstaller() {
    let fixture = Fixture::new("install");
    let report = install(&fixture.options()).expect("install");
    assert!(report.app.is_file());
    assert!(report.cli.is_file());
    assert!(report.uninstaller.is_file());
    let script = fs::read_to_string(report.uninstaller).expect("uninstall script");
    assert!(script.contains("Remove-Item"));
}

#[test]
fn uninstall_removes_the_installed_directory() {
    let fixture = Fixture::new("uninstall");
    let options = fixture.options();
    install(&options).expect("install");
    uninstall(&options.target).expect("uninstall");
    assert!(!options.target.exists());
}

#[test]
fn missing_package_file_is_reported() {
    let fixture = Fixture::new("missing");
    fs::remove_file(fixture.root.join("package").join("atk-hub-cli.exe")).expect("remove");
    assert!(matches!(
        install(&fixture.options()),
        Err(InstallError::MissingFile { .. })
    ));
}
