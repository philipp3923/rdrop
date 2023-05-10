use std::mem::replace;
use std::net::Ipv6Addr;
use std::ops::Deref;
use std::str::FromStr;
use std::sync::{Arc, mpsc, Mutex, RwLock};
use std::sync::mpsc::{channel, Sender, SyncSender};
use std::thread;
use std::time::Duration;
use tauri::State;
use p2p::client::{EncryptedReader, EncryptedWriter, WaitingClient};
use p2p::client::tcp::{TcpClientReader, TcpClientWriter};
use p2p::client::udp::{UdpClientReader, UdpClientWriter};
use p2p::error::{ChangeStateError, Error};
use p2p::protocol::{Active, Connection, Encrypted, Plain, Tcp, Udp, Waiting};
use crate::client::Client;
use crate::error::{ClientError, ClientErrorKind};
use crate::handle::Current::Connected;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(1);
const DISCONNECT_TIMEOUT: Duration = Duration::from_secs(5);


pub struct AppState(RwLock<Current>);

impl AppState {
    pub fn new() -> Self {
        AppState(RwLock::new(Current::new()))
    }
}


pub enum Current {
    Broken,
    Disconnected(Connection<Waiting>),
    Connecting(SyncSender<()>),
    Connected,
    Connected2(Client<EncryptedWriter<TcpClientWriter>, EncryptedReader<TcpClientReader>>)
}

impl Current {

    pub fn new() -> Self {
        match Connection::new(None) {
            Ok(c) => Current::Disconnected(c),
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
pub fn connect(state: State<AppState>, ip: String, port: u16) {
    //testable_connect(state.deref(), ip, port).unwrap()

}
pub fn testable_connect(state: Arc<AppState>, ip: String, port: u16) -> Result<(), ClientError> {
    let ipv6 = match Ipv6Addr::from_str(&*ip) {
        Ok(c) => c,
        Err(_) => {
            return Err(ClientError::new(ClientErrorKind::Ipv6ParseFailed));
        },
    };

    let mut write_state = state.0.write().unwrap();

    let (sender, receiver) = mpsc::sync_channel::<()>(0);

    let mut connection = match write_state.deref() {
        Current::Disconnected(connection) => {
            let prev_state = replace(&mut *write_state, Current::Connecting(sender));

            match prev_state {
                Current::Disconnected(connection) => {connection},
                _ => {
                    return Err(ClientError::new(ClientErrorKind::WrongState))
                }
            }
        },
        _ => {
            return Err(ClientError::new(ClientErrorKind::WrongState))
        }
    };

    drop(write_state);

    //#TODO send status connecting

    thread::spawn(move || {
        println!("thread anfang");
        let mut i = 0;
        while receiver.try_recv().is_err() {
            i+= 1;
            println!("next {i}");

            match connection.connect(ipv6, port, Some(DEFAULT_TIMEOUT), Some(DISCONNECT_TIMEOUT)) {
                Ok(active_connection) => {
                    //#TODO send status encrypting
                    let active_connection = match active_connection.encrypt() {
                        Ok(connection) => connection,
                        Err(err) => {
                            //#TODO send status error on encrypt
                            let active_connection = err.to_state();
                            let mut write_state = state.0.write().unwrap();

                            *write_state = Current::try_with_port(active_connection.get_port());
                            return;
                        }
                    };

                    //#TODO send status upgrading tcp

                    let active_connection = match active_connection.upgrade() {
                        Ok(connection) => {
                            let mut write_state = state.0.write().unwrap();

                            *write_state = Current::Connected;
                        }
                        Err(err) => {
                            //#TODO send status tcp failed
                            let active_connection = err.to_state();

                            let mut write_state = state.0.write().unwrap();

                            *write_state = Current::Connected;
                        }
                    };

                    return;
                }
                Err(err) => {
                    connection = err.to_state();
                }
            }
        }

        println!("test ende");
        let mut write_state = state.0.write().unwrap();

        *write_state = Current::Disconnected(connection);
    });

    return Ok(());
}

#[tauri::command]
pub fn disconnect(state: State<AppState>) {
    let mut write_state = (*state).0.write().unwrap();

    match write_state.deref() {
        Current::Connecting(sender) => {
            sender.send(()).unwrap()
        },
        _ => {
            todo!()
        }
    }
}

#[tauri::command]
pub fn offer_file(state: State<AppState>, path: String) {

}

#[tauri::command]
pub fn accept_file(state: State<AppState>, hash: String) {

}

#[tauri::command]
pub fn deny_file(state: State<AppState>, hash: String) {

}

#[tauri::command]
pub fn pause_file(state: State<AppState>, hash: String) {

}

#[cfg(test)]
mod tests {
    use std::sync::{LockResult, RwLockReadGuard};
    use std::thread::sleep;
    use super::*;

    #[test]
    fn test_connect() {
        let state = Arc::new(AppState::new());
        testable_connect(state.clone(), "0:0:0:0:0:0:0:1".to_string(), 1000).unwrap();

        sleep(Duration::from_millis(50));
        let r_state = state.0.read().unwrap();
        println!("stopping now");
        match r_state.deref() {
            Current::Connecting(sender) => {
                sender.send(()).unwrap();
            }
            _ => {
                panic!()
            }
        }

        drop(r_state);

        sleep(Duration::from_secs(1));


        let r_state = state.0.read().unwrap();
        assert!(match r_state.deref() {
            Current::Disconnected(_) => true,
            _ => false
        });
        drop(r_state);

        testable_connect(state.clone(), "0:0:0:0:0:0:0:1".to_string(), 1000).unwrap();

        sleep(Duration::from_millis(50));
        let r_state = state.0.read().unwrap();
        println!("stopping now");
        match r_state.deref() {
            Current::Connecting(sender) => {
                sender.send(()).unwrap();
            }
            _ => {
                panic!()
            }
        }

        drop(r_state);

        sleep(Duration::from_secs(1));


        let r_state = state.0.read().unwrap();
        assert!(match r_state.deref() {
            Current::Disconnected(_) => true,
            _ => false
        });
    }

}