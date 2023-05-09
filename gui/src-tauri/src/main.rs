#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use tauri::{AppHandle, Manager, Wry};
use window_shadows::set_shadow;
use std::time::Duration;
use std::thread::sleep;
use p2p::error::Error;
use p2p::protocol::{Connection, Waiting};

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
            match set_shadow(&window, true) {
                Ok(_) => {println!("WINDOWS")}
                Err(_) => {println!("LINUX")}
            };

            match Connection::new(None) {
                Ok(connection) => run(handle, connection),
                Err(err) => run_err(handle, err)
            }

            /*app.listen_global("app://add-file", |event| {
                println!("got app://add-file with payload {:?}", event.payload());
            });

            app.listen_global("app://start", move |event| {
                event_handler.send_update_status("Connecting", "", false);
                sleep(Duration::new(2, 0));
                event_handler.send_update_status("Punshing holes", "Real deep", false);
                sleep(Duration::new(2, 0));
                event_handler.send_update_status("Failed to establish connection", "Timeout", true);
            });*/
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn run_err(handle: AppHandle<Wry>, err: p2p::error::Error) {
    let event_handler = EventHandler::new(handle.clone());
}

fn run(handle: AppHandle<Wry>, connection: Connection<Waiting>) {
    let event_handler = EventHandler::new(handle.clone());
}

struct EventHandler {
    handle: AppHandle<Wry>,
}

impl EventHandler {
    fn new(handle: AppHandle<Wry>) -> Self {
        Self { handle }
    }

    fn send_connect_status(&self, status: &str, description: &str) {
        self.handle
            .emit_all("app://update-status", Status { status: status.into(), description: description.into(), error: false})
            .unwrap();
    }

    fn send_connect_error(&self, status: &str, description: &str) {
        self.handle
            .emit_all("app://update-status", Status { status: status.into(), description: description.into(), error: false})
            .unwrap();
    }

    fn send_init_error(&self, status: &str, description: &str) {
        self.handle
            .emit_all("app://update-status", Status { status: status.into(), description: description.into(), error: false})
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