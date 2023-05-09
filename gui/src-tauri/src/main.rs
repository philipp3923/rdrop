#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::error::Error;
use std::net::Ipv6Addr;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use tauri::{App, AppHandle, Manager, Wry};
use window_shadows::set_shadow;
use std::time::Duration;
use std::thread::sleep;
use p2p::error::{ChangeStateError, Error as P2pError};
use p2p::protocol::{Active, Connection, Encrypted, Plain, Tcp, Udp, Waiting};
use serde::Deserialize;

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}


const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(60);
const DEFAULT_DISCONNECT_TIMEOUT: Duration = Duration::from_secs(5);

fn main() {

    tauri::Builder::default()
        .setup(|app| {
            let handle = app.handle();
            let window = app.get_window("main").unwrap();
            match set_shadow(&window, true) {
                Ok(_) => {println!("WINDOWS")}
                Err(_) => {println!("LINUX")}
            };

            let event_handler : Arc<Mutex<EventHandler>> = Arc::new(Mutex::new(EventHandler::new(handle.clone())));
            let waiting_connection: Arc<Mutex<Option<Connection<Waiting>>>> = Arc::new(Mutex::new(None));
            let active_connection: Arc<Mutex<Option<Connection<Active<Encrypted<Tcp>>>>>> = Arc::new(Mutex::new(None));

            register_start_event(app, event_handler.clone(), waiting_connection.clone());
            register_connect_event(app, event_handler.clone(), waiting_connection.clone());

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

fn register_start_event(app: &mut App<Wry>, event_handler : Arc<Mutex<EventHandler>>, waiting_connection: Arc<Mutex<Option<Connection<Waiting>>>>) {
    app.listen_global("app://start", move |event| {
        let event_handler = event_handler.lock().unwrap();
        let mut waiting_connection = waiting_connection.lock().unwrap();

        match Connection::new(None) {
            Ok(connection) => {
                event_handler.send_bind_port(connection.get_port());
                println!("port {}", connection.get_port());
                waiting_connection.replace(connection);
            }
            Err(err) => {
                event_handler.send_init_error("Initialization failed.", format!("Error Code {:?}", err.kind()).as_str());
            }
        }
    });
}

fn register_connect_event(app: &mut App<Wry>, event_handler : Arc<Mutex<EventHandler>>, waiting_connection: Arc<Mutex<Option<Connection<Waiting>>>>) {
    app.listen_global("app://connect", move |event| {
        let event_handler = event_handler.lock().unwrap();
        let mut waiting_connection = waiting_connection.lock().unwrap();

        let waiting_connection_unlocked = match waiting_connection.take() {
            None => {
                println!("illegal connect event.");
                return;
            }
            Some(connection) => connection,
        };

        let connect_payload: ConnectPayload = match serde_json::from_str(event.payload().unwrap_or("")) {
            Ok(c) => c,
            Err(_) => {
                println!("json parse at connect failed");
                return;
            },
        };

        let port = match u16::from_str(&connect_payload.port) {
            Ok(c) => c,
            Err(_) => {
                println!("parsing port failed at connect");
                waiting_connection.replace(waiting_connection_unlocked);
                return;
            },
        };

        let ipv6 = match Ipv6Addr::from_str(&connect_payload.ip) {
            Ok(c) => c,
            Err(_) => {
                println!("parsing ipv6 failed at connect");
                waiting_connection.replace(waiting_connection_unlocked);
                return;
            },
        };

        println!("{:?}", connect_payload);
        event_handler.send_connect_status("Connecting", "");

        let result = waiting_connection_unlocked.connect(ipv6, port, None, None).unwrap();
        /*{
            Ok(active_connection) => {
                event_handler.send_connect_status("Connected!!!", "");
            }
            Err(err) => {
                let (connection, err) = err.split();
                event_handler.send_connect_error("Connecting failed.", err.as_ref().to_string().as_str());
                waiting_connection.replace(connection);
            }
        }*/
        drop(result);

    });
}

struct EventHandler {
    handle: AppHandle<Wry>,
}

impl EventHandler {
    fn new(handle: AppHandle<Wry>) -> Self {
        Self { handle }
    }

    fn send_bind_port(&self, port: u16) {
        self.handle
            .emit_all("app://update-port", port)
            .unwrap();
    }

    fn send_connect_status(&self, status: &str, description: &str) {
        self.handle
            .emit_all("app://update-status", Status { status: status.into(), description: description.into(), error: false})
            .unwrap();
    }

    fn send_connect_error(&self, status: &str, description: &str) {
        self.handle
            .emit_all("app://update-status", Status { status: status.into(), description: description.into(), error: true})
            .unwrap();
    }

    fn send_init_error(&self, status: &str, description: &str) {
        self.handle
            .emit_all("app://socket-failed", Status { status: status.into(), description: description.into(), error: true})
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

#[derive(Deserialize, Debug)]
struct ConnectPayload {
    ip: String,
    port: String // serde ist leider zu eingeschränkt um eine json number automatisch in u16 zu parsen...
}