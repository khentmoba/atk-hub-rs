//! Command line front end for the license-free ATK HUB installer.

use atk_installer::{
    InstallError, InstallOptions, LOGO_NAME, bootstrap, default_target, install, log_line,
    show_error, show_logo, uninstall,
};
use std::path::{Path, PathBuf};

fn print_help() {
    println!(
        "ATK HUB Rust port — license-free installer\n\n\
         usage: setup.exe [options]\n\
           (no options)          install to %LOCALAPPDATA%\\AtkVhub with shortcut\n\
           --target <path>       install to a custom directory\n\
           --source <path>       use a different portable package directory\n\
           --no-shortcut         skip Start Menu shortcut\n\
           --silent              skip the logo window\n\
           --uninstall           remove the installed copy\n\
           --uninstall --target <path>  remove a custom installation\n\
           --help                show this help\n"
    );
}

fn run() -> Result<(), InstallError> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help") {
        print_help();
        return Ok(());
    }
    let uninstalling = args.iter().any(|arg| arg == "--uninstall");
    let mut source = std::env::current_exe()?
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();
    let mut target = default_target()?;
    let mut shortcut = true;
    let mut silent = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--target" if index + 1 < args.len() => {
                target = PathBuf::from(&args[index + 1]);
                index += 1;
            }
            "--source" if index + 1 < args.len() => {
                source = PathBuf::from(&args[index + 1]);
                index += 1;
            }
            "--no-shortcut" => shortcut = false,
            "--silent" => silent = true,
            _ => {}
        }
        index += 1;
    }

    if uninstalling {
        uninstall(&target)?;
        log_line(&format!("removed {}", target.display()));
        println!("removed {}", target.display());
        return Ok(());
    }

    let source = bootstrap(&source)?;
    if !silent {
        show_logo(&source.join(LOGO_NAME));
    }
    log_line(&format!("installing from {}", source.display()));
    let report = install(&InstallOptions {
        source,
        target,
        shortcut,
    })?;
    log_line(&format!("installed {}", report.app.display()));
    println!("installed: {}", report.app.display());
    println!("cli: {}", report.cli.display());
    println!("uninstaller: {}", report.uninstaller.display());
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        log_line(&format!("ERROR: {error}"));
        eprintln!("{error}");
        show_error(&error.to_string());
        std::process::exit(1);
    }
}
