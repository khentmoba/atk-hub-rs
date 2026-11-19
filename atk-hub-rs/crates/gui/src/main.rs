//! Native entry point for the ATK HUB Tauri shell.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    atk_gui::run();
}
