#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use tauri::Manager;
use window_shadows::set_shadow;
use std::time::Duration;
use std::thread::sleep;

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let handle = app.handle();
            let window = app.get_window("main").unwrap();
            set_shadow(&window, true).expect("Unsupported platform!");

            app.listen_global("app://add-file", |event| {
                println!("got app://add-file with payload {:?}", event.payload());
            });

            app.listen_global("app://start", move |event| {
                sleep(Duration::new(1, 0));
                handle.emit_all("app://update-status", "Encrypting").unwrap();
                sleep(Duration::new(1, 0));
                handle.emit_all("app://update-status", "Punching holes").unwrap();
                sleep(Duration::new(1, 0));
                handle.emit_all("app://update-status", "Doing literally nothing").unwrap();
                sleep(Duration::new(1, 0));
                handle.emit_all("app://update-status", "SHEEESSHHH").unwrap();
            });


            Ok(())
        })
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
