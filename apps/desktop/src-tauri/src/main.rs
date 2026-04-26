// Prevents additional console window on Windows in release; OS-aware in debug.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    holster_desktop_lib::run();
}
