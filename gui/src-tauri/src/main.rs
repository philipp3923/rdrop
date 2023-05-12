#![cfg_attr(
all(not(debug_assertions), target_os = "windows"),
windows_subsystem = "windows"
)]

use std::error::Error;
use std::net::Ipv6Addr;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;

use serde::Deserialize;
use tauri::{App, AppHandle, Manager, Wry};
use window_shadows::set_shadow;

use p2p::error::{ChangeStateError, Error as P2pError};
use p2p::protocol::{Active, Connection, Encrypted, Plain, Tcp, Udp, Waiting};

use crate::handle::{AppState, Current};

mod client;
mod handle;
mod error;
mod events;
mod connect;

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}


fn main() {
    tauri::Builder::default()
        .manage(AppState::new())
        .setup(|app| {
            let handle = app.handle();
            let window = app.get_window("main").unwrap();
            match set_shadow(&window, true) {
                Ok(_) => { println!("WINDOWS") }
                Err(_) => { println!("LINUX") }
            };

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![greet, handle::connect, handle::disconnect, handle::offer_file, handle::accept_file, handle::deny_file, handle::pause_file, handle::start])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}