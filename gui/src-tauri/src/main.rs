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
            let event_handler = EventHandler::new(handle.clone());
            let window = app.get_window("main").unwrap();
            set_shadow(&window, true).expect("Unsupported platform!");

            app.listen_global("app://add-file", |event| {
                println!("got app://add-file with payload {:?}", event.payload());
            });

            app.listen_global("app://start", move |event| {
                event_handler.send_update_status("Connecting", "", false);
                sleep(Duration::new(2, 0));
                event_handler.send_update_status("Punshing holes", "Real deep", false);
                sleep(Duration::new(2, 0));
                event_handler.send_update_status("Failed to establish connection", "Timeout", true);
            });


            Ok(())
        })
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

struct EventHandler {
    handle: tauri::AppHandle,
}

impl EventHandler {
    fn new(handle: tauri::AppHandle) -> Self {
        Self { handle }
    }

    fn send_update_status(&self, status: &str, description: &str, error: bool) {
        self.handle
            .emit_all("app://update-status", Status { status: status.into(), description: description.into(), error: error})
            .unwrap();
    }
}

#[derive(Clone, serde::Serialize)]
struct Status {
    status: String,
    description: String,
    error: bool,
}

struct FileEntry {
    id: String,
    name: String,
    size: u64,
    is_sender: bool,
}