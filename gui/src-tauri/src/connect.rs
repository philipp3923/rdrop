use std::error::Error;
use std::net::Ipv6Addr;
use std::ops::Deref;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tauri::{AppHandle, State, Wry};
use p2p::error::{ErrorKind};
use p2p::protocol::{Connection, Waiting};
use crate::error::{ClientError, ClientErrorKind};
use crate::events::{send_connect_error, send_connect_status};
use crate::handle::{AppState, Current};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(1);
const DISCONNECT_TIMEOUT: Duration = Duration::from_secs(5);

pub fn thread_connect(app_handle: AppHandle<Wry>, current: Arc<Mutex<Current>>, mut connection: Connection<Waiting>, receiver: Receiver<()>, ipv6: Ipv6Addr, port: u16) -> Result<(), ClientError>{
    let mut i = 0;
    while receiver.try_recv().is_err() {
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

                send_connect_status(&app_handle, "Upgrading", "Changing UDP protocol to TCP.")?;


                match active_connection.upgrade() {
                    Ok(connection) => {
                        let mut write_state = current.lock().unwrap();
                        send_connect_status(&app_handle, "Connected successfully", "")?;
                        *write_state = Current::Connected;
                    }
                    Err(err) => {
                        send_connect_status(&app_handle, "Upgrading failed", "Using fallback UDP protocol.")?;

                        let active_connection = err.to_state();

                        let mut write_state = current.lock().unwrap();

                        *write_state = Current::Connected;
                    }
                };

                return Ok(());
            }
            Err(err) => {
                let (old_c, err) = err.split();
                connection = old_c;

                match err.downcast_ref::<p2p::error::Error>() {
                    Some(err) => {
                        match err.kind() {
                            ErrorKind::CannotConnectToSelf => {
                                send_connect_error(&app_handle,"Cannot connect to self", "")?;
                                break
                            },
                            _ => continue,
                        }
                    }
                    None => {}
                }

            }
        }
    }

    let mut write_state = current.lock().unwrap();

    *write_state = Current::Disconnected(connection);

    return Err(ClientError::new(ClientErrorKind::SocketClosed))
}