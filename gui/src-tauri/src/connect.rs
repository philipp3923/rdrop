use std::error::Error;
use std::net::Ipv6Addr;
use std::ops::Deref;
use std::sync::{Arc, Mutex, MutexGuard};
use std::sync::mpsc::Receiver;
use std::thread::sleep;
use std::time::{Duration, Instant};

use tauri::{AppHandle, State, Wry};
use tauri::async_runtime::handle;

use p2p::error::ErrorKind;
use p2p::protocol::{Connection, Waiting};

use crate::client::Client;
use crate::error::{ClientError, ClientErrorKind};
use crate::events::{Protocol, send_connect_error, send_connect_status, send_connected};
use crate::handle::{AppState, Current};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(1);
const DISCONNECT_TIMEOUT: Duration = Duration::from_secs(5);

pub fn thread_connect(app_handle: AppHandle<Wry>, current: Arc<Mutex<Current>>, mut connection: Connection<Waiting>, receiver: Receiver<()>, ipv6: Ipv6Addr, port: u16) -> Result<(), ClientError> {
    let mut i = 0;
    let mut instant = Instant::now();
    while receiver.try_recv().is_err() {
        if instant.elapsed() < Duration::from_millis(50){
            sleep(Duration::from_millis(51) - instant.elapsed());
        }
        instant = Instant::now();
        i += 1;
        println!("next {i}");

        match connection.connect(ipv6, port, Some(DEFAULT_TIMEOUT), Some(DISCONNECT_TIMEOUT)) {
            Ok(active_connection) => {
                send_connect_status(&app_handle, "Encrypting", "Securing the connection.")?;

                let active_connection = match active_connection.encrypt() {
                    Ok(connection) => connection,
                    Err(err) => {
                        send_connect_error(&app_handle, "Encryption failed", "Aborting connection protocol.")?;

                        let active_connection = err.to_state();
                        let mut write_state = current.lock().unwrap();

                        *write_state = Current::try_with_port(active_connection.get_port());
                        return Err(ClientError::new(ClientErrorKind::SocketClosed));
                    }
                };

                send_connect_status(&app_handle, "Upgrading", "Sampling time difference.")?;

                let active_connection = match active_connection.upgrade() {
                    Ok(connection) => {
                        let mut write_state = current.lock().unwrap();
                        send_connect_status(&app_handle, "Connected successfully", "")?;
                        let port = connection.get_port();

                        let (writer, reader) = connection.accept();
                        let client = Client::new(app_handle.clone(), reader, writer, Some(DISCONNECT_TIMEOUT), port);

                        *write_state = Current::ConnectedTcp(client);
                        send_connected(&app_handle, Protocol::TCP)?;
                        return Ok(());
                    }
                    Err(err) => {
                        send_connect_status(&app_handle, "Upgrading failed", "Using fallback UDP protocol.")?;

                        let (old_connection, err) = err.split();

                        println!("{}", err);

                        old_connection
                    }
                };

                send_connect_status(&app_handle, "Upgrading", "Synchronizing using NTP server.")?;

                match active_connection.upgrade_using_ntp() {
                    Ok(connection) => {
                        let mut write_state = current.lock().unwrap();
                        send_connect_status(&app_handle, "Connected successfully", "")?;
                        let port = connection.get_port();

                        let (writer, reader) = connection.accept();
                        let client = Client::new(app_handle.clone(), reader, writer, Some(DISCONNECT_TIMEOUT), port);

                        *write_state = Current::ConnectedTcp(client);
                        send_connected(&app_handle, Protocol::TCP)?;
                    }
                    Err(err) => {
                        send_connect_status(&app_handle, "Upgrading failed", "Using fallback UDP protocol.")?;

                        let (connection, err) = err.split();

                        println!("{}", err);

                        let mut write_state = current.lock().unwrap();

                        let (writer, reader) = connection.accept();
                        let client = Client::new(app_handle.clone(), reader, writer, Some(DISCONNECT_TIMEOUT), port);

                        *write_state = Current::ConnectedUdp(client);
                        send_connected(&app_handle, Protocol::UDP)?;
                    }
                }

                return Ok(());
            }
            Err(err) => {
                let (old_c, err) = err.split();
                connection = old_c;

                match err.downcast_ref::<p2p::error::Error>() {
                    Some(err) => {
                        match err.kind() {
                            ErrorKind::CannotConnectToSelf => {
                                send_connect_error(&app_handle, "Cannot connect to self", "")?;
                                break;
                            }
                            _ => continue,
                        }
                    }
                    None => {}
                }
            }
        }
    }

    println!("Dummer hs");

    let mut write_state = current.lock().unwrap();

    *write_state = Current::Disconnected(connection);

    return Err(ClientError::new(ClientErrorKind::SocketClosed));
}