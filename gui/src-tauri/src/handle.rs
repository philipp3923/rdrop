
use std::fs::metadata;
use std::mem::replace;
use std::net::Ipv6Addr;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::process::Command;
use std::str::FromStr;
use std::sync::mpsc::{SyncSender};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;


use tauri::{AppHandle, State, Wry};

use p2p::client::tcp::{TcpClientReader, TcpClientWriter};
use p2p::client::udp::{UdpClientReader, UdpClientWriter};
use p2p::client::{EncryptedReader, EncryptedWriter, WaitingClient};

use p2p::protocol::{Connection, Waiting};

use crate::client::Client;
use crate::connect::thread_connect;
use crate::error::{ClientError, ClientErrorKind};
use crate::events::{send_bind_port, send_connect_status};

pub struct AppState(Arc<Mutex<Current>>);

impl AppState {
    pub fn new() -> Self {
        AppState(Arc::new(Mutex::new(Current::new())))
    }

    pub fn current(&self) -> &Arc<Mutex<Current>> {
        &self.0
    }
}

pub enum Current {
    Broken,
    Disconnected(Connection<Waiting>),
    Connecting(SyncSender<()>),
    ConnectedUdp(Client<EncryptedWriter<UdpClientWriter>, EncryptedReader<UdpClientReader>>),
    ConnectedTcp(Client<EncryptedWriter<TcpClientWriter>, EncryptedReader<TcpClientReader>>),
}

impl Current {
    pub fn new() -> Self {
        match Connection::new(None) {
            Ok(c) => {
                println!("init port : {}", c.get_port());
                Current::Disconnected(c)
            }
            Err(_) => Current::Broken,
        }
    }

    pub fn try_with_port(port: u16) -> Self {
        match Connection::new(Some(port)) {
            Ok(c) => Current::Disconnected(c),
            Err(_) => Self::new(),
        }
    }
}

#[tauri::command]
pub fn connect(
    app_handle: AppHandle<Wry>,
    app_state: State<AppState>,
    ip: String,
    port: u16,
) -> Result<(), ClientError> {
    let ipv6 = match Ipv6Addr::from_str(&*ip) {
        Ok(c) => c,
        Err(_) => {
            return Err(ClientError::new(ClientErrorKind::Ipv6ParseFailed));
        }
    };

    let mut unlocked_state = app_state.0.lock().unwrap();

    let (sender, receiver) = mpsc::sync_channel::<()>(0);

    let connection = match unlocked_state.deref() {
        Current::Disconnected(_) => {
            let prev_state = replace(&mut *unlocked_state, Current::Connecting(sender));

            match prev_state {
                Current::Disconnected(connection) => connection,
                _ => {
                    return Err(ClientError::new(ClientErrorKind::WrongState));
                }
            }
        }
        _ => {
            return Err(ClientError::new(ClientErrorKind::WrongState));
        }
    };

    drop(unlocked_state);

    send_connect_status(&app_handle, "Connecting", "Waiting for response from peer.")?;

    let current: Arc<Mutex<Current>> = app_state.current().clone();
    thread::spawn(move || thread_connect(app_handle, current, connection, receiver, ipv6, port));

    return Ok(());
}

#[tauri::command]
pub fn disconnect(
    app_handle: AppHandle<Wry>,
    app_state: State<AppState>,
) -> Result<(), ClientError> {
    println!("[EVENT] Disconnect");
    let unlocked_state = (*app_state).0.lock()?;

    match unlocked_state.deref() {
        Current::Connecting(sender) => {
            println!("Connecting");
            sender.send(())?;
            drop(unlocked_state);
            start(app_handle, app_state)
        }
        _ => {
            println!("Other");
            drop(unlocked_state);
            start(app_handle, app_state)
        }
    }
}

#[tauri::command]
pub fn offer_file(app_state: State<AppState>, path: String) -> Result<(), ClientError> {
    println!("[EVENT] offer_file");
    let mut unlocked_state = (*app_state).0.lock()?;

    match unlocked_state.deref_mut() {
        &mut Current::ConnectedUdp(ref mut client) => client.offer_file(path),
        &mut Current::ConnectedTcp(ref mut client) => client.offer_file(path),
        _ => Err(ClientError::new(ClientErrorKind::WrongState)),
    }
}

#[tauri::command]
pub fn accept_file(
    app_state: State<AppState>,
    hash: String,
    path: String,
) -> Result<(), ClientError> {
    println!("[EVENT] accept_file");
    let mut unlocked_state = (*app_state).0.lock()?;

    match unlocked_state.deref_mut() {
        &mut Current::ConnectedUdp(ref mut client) => client.accept_file(hash, path),
        &mut Current::ConnectedTcp(ref mut client) => client.accept_file(hash, path),
        _ => Err(ClientError::new(ClientErrorKind::WrongState)),
    }
}

#[tauri::command]
pub fn deny_file(app_state: State<AppState>, hash: String) -> Result<(), ClientError> {
    println!("[EVENT] deny_file");
    let mut unlocked_state = (*app_state).0.lock()?;

    match unlocked_state.deref_mut() {
        &mut Current::ConnectedUdp(ref mut client) => client.deny_file(hash),
        &mut Current::ConnectedTcp(ref mut client) => client.deny_file(hash),
        _ => Err(ClientError::new(ClientErrorKind::WrongState)),
    }
}

#[tauri::command]
pub fn stop_file(app_state: State<AppState>, hash: String) -> Result<(), ClientError> {
    println!("[EVENT] stop_file");
    let mut unlocked_state = (*app_state).0.lock()?;

    match unlocked_state.deref_mut() {
        &mut Current::ConnectedUdp(ref mut client) => client.stop_file(hash),
        &mut Current::ConnectedTcp(ref mut client) => client.stop_file(hash),
        _ => Err(ClientError::new(ClientErrorKind::WrongState)),
    }
}

#[tauri::command]
pub fn pause_file(app_state: State<AppState>, hash: String) -> Result<(), ClientError> {
    println!("[EVENT] pause_file");
    let mut unlocked_state = (*app_state).0.lock()?;

    match unlocked_state.deref_mut() {
        &mut Current::ConnectedUdp(ref mut client) => client.pause_file(hash),
        &mut Current::ConnectedTcp(ref mut client) => client.pause_file(hash),
        _ => Err(ClientError::new(ClientErrorKind::WrongState)),
    }
}

#[tauri::command]
pub fn start(app_handle: AppHandle<Wry>, app_state: State<AppState>) -> Result<(), ClientError> {
    println!("[EVENT] start");
    let mut unlocked_state = (*app_state).0.lock()?;

    match unlocked_state.deref() {
        Current::Disconnected(c) => {
            send_bind_port(&app_handle, c.get_port())?;
            println!("port: {}", c.get_port());
            Ok(())
        }
        Current::ConnectedUdp(c) => {
            println!("ConnectedUdp");
            let port = c.get_port();

            let old_state = replace(&mut *unlocked_state,Current::Broken);
            drop(old_state);

            *unlocked_state = Current::try_with_port(port);
            drop(unlocked_state);
            start(app_handle, app_state)
        }
        Current::ConnectedTcp(c) => {
            println!("ConnectedTcp");
            let port = c.get_port();

            let old_state = replace(&mut *unlocked_state,Current::Broken);
            drop(old_state);

            *unlocked_state = Current::try_with_port(port);
            drop(unlocked_state);
            start(app_handle, app_state)
        }
        _ => {
            println!("Other");
            *unlocked_state = Current::new();
            drop(unlocked_state);
            start(app_handle, app_state)
        }
    }
}

#[tauri::command]
pub fn show_in_folder(path: String) {
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .args(["/select,", &path]) // The comma after select is not a typo
            .spawn()
            .unwrap();
    }

    #[cfg(target_os = "linux")]
    {
        if path.contains(",") {
            // see https://gitlab.freedesktop.org/dbus/dbus/-/issues/76
            let new_path = match metadata(&path).unwrap().is_dir() {
                true => path,
                false => {
                    let mut path2 = PathBuf::from(path);
                    path2.pop();
                    path2.into_os_string().into_string().unwrap()
                }
            };
            Command::new("xdg-open").arg(&new_path).spawn().unwrap();
        } else {
            Command::new("dbus-send")
                .args([
                    "--session",
                    "--dest=org.freedesktop.FileManager1",
                    "--type=method_call",
                    "/org/freedesktop/FileManager1",
                    "org.freedesktop.FileManager1.ShowItems",
                    format!("array:string:file://{path}").as_str(),
                    "string:\"\"",
                ])
                .spawn()
                .unwrap();
        }
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open").args(["-R", &path]).spawn().unwrap();
    }
}
