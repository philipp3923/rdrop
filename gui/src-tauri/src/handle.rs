use std::io::SeekFrom::Current;
use std::mem::replace;
use std::net::Ipv6Addr;
use std::ops::Deref;
use std::str::FromStr;
use std::sync::{Arc, mpsc, Mutex, RwLock};
use std::sync::mpsc::{channel, Sender, SyncSender};
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, State, Wry};

use p2p::client::{EncryptedReader, EncryptedWriter, WaitingClient};
use p2p::client::tcp::{TcpClientReader, TcpClientWriter};
use p2p::client::udp::{UdpClientReader, UdpClientWriter};
use p2p::error::{ChangeStateError, Error};
use p2p::protocol::{Active, Connection, Encrypted, Plain, Tcp, Udp, Waiting};

use crate::client::Client;
use crate::connect::thread_connect;
use crate::error::{ClientError, ClientErrorKind};
use crate::events::{send_bind_port, send_connect_status};
use crate::handle::Current::Connected;

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
    Connected,
    Connected2(Client<EncryptedWriter<TcpClientWriter>, EncryptedReader<TcpClientReader>>),
}

impl Current {
    pub fn new() -> Self {
        match Connection::new(None) {
            Ok(c) => {
                println!("init port : {}", c.get_port());
                Current::Disconnected(c)
            },
            Err(_) => Current::Broken
        }
    }

    pub fn try_with_port(port: u16) -> Self {
        match Connection::new(Some(port)) {
            Ok(c) => Current::Disconnected(c),
            Err(_) => Self::new()
        }
    }
}


#[tauri::command]
pub fn connect(app_handle: AppHandle<Wry>, app_state: State<AppState>, ip: String, port: u16) -> Result<(), ClientError> {
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
                Current::Disconnected(connection) => { connection }
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
    thread::spawn(move || {
        thread_connect(app_handle, current, connection, receiver, ipv6, port)
    });

    return Ok(());
}

#[tauri::command]
pub fn disconnect(state: State<AppState>) -> Result<(), ClientError> {
    let mut unlocked_state = (*state).0.lock().unwrap();

    match unlocked_state.deref() {
        Current::Connecting(sender) => {
            sender.send(())?;
            Ok(())
        }
        _ => {
            *unlocked_state = Current::new();
            Ok(())
        }
    }
}

#[tauri::command]
pub fn offer_file(state: State<AppState>, path: String) {}

#[tauri::command]
pub fn accept_file(state: State<AppState>, hash: String) {}

#[tauri::command]
pub fn deny_file(state: State<AppState>, hash: String) {}

#[tauri::command]
pub fn pause_file(state: State<AppState>, hash: String) {}

#[tauri::command]
pub fn start(app_handle: AppHandle<Wry>, app_state: State<AppState>) -> Result<u16, ClientError> {
    let app_state = (*app_state).0.lock()?;

    match app_state.deref() {
        Current::Disconnected(c) => {
            send_bind_port(&app_handle, c.get_port())?;
            println!("port: {}", c.get_port());
            Ok(c.get_port())
        },
        _ => Err(ClientError::new(ClientErrorKind::WrongState))
    }
}