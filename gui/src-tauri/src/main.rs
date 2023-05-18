#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use tauri::Manager;
use window_shadows::set_shadow;
use crate::handle::AppState;

mod client;
mod connect;
mod error;
mod events;
mod handle;


fn main() {
    tauri::Builder::default()
        .manage(AppState::new())
        .setup(|app| {
            let _handle = app.handle();
            let window = app.get_window("main").unwrap();
            match set_shadow(&window, true) {
                Ok(_) => {
                    println!("WINDOWS")
                }
                Err(_) => {
                    println!("LINUX")
                }
            };

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            handle::connect,
            handle::disconnect,
            handle::offer_file,
            handle::accept_file,
            handle::deny_file,
            handle::pause_file,
            handle::start,
            handle::show_in_folder,
            handle::stop_file
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
