#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Err(error) = media_manager::run() {
        eprintln!("Media Manager could not start: {error}");
        std::process::exit(1);
    }
}
