use std::net::Ipv6Addr;

use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Wry};

use p2p::error::ErrorKind;
use p2p::protocol::{Connection, Waiting};

use crate::client::Client;
use crate::error::{ClientError, ClientErrorKind};
use crate::events::{send_connect_error, send_connect_status, send_connected, Protocol};
use crate::handle::Current;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(1);
const DISCONNECT_TIMEOUT: Duration = Duration::from_secs(5);


/// Establishes a connection with a remote peer.
///
/// # Arguments
///
/// * `app_handle` - Handle for the tauri application.
/// * `current` - The current state of the client.
/// * `connection` - The waiting connection, which now should be connected.
/// * `receiver` - A Receiver for receiving termination signals.
/// * `ipv6` - The IPv6 address of the remote server.
/// * `port` - The port number of the remote server.
///
/// # Returns
///
/// Returns `Ok(())` if the connection is established successfully, or an `Err` containing a `ClientError`
/// if an error occurs during the connection process.
pub fn thread_connect(
    app_handle: AppHandle<Wry>,
    current: Arc<Mutex<Current>>,
    mut connection: Connection<Waiting>,
    receiver: Receiver<()>,
    ipv6: Ipv6Addr,
    port: u16,
) -> Result<(), ClientError> {
    let mut i = 0;
    let mut instant = Instant::now();
    let self_port = connection.get_port();
    while receiver.try_recv().is_err() {
        if instant.elapsed() < Duration::from_millis(50) {
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
                        send_connect_error(
                            &app_handle,
                            "Encryption failed",
                            "Aborting connection protocol.",
                        )?;

                        let active_connection = err.to_state();
                        let mut write_state = current.lock().unwrap();

                        *write_state = Current::try_with_port(active_connection.get_port());
                        return Err(ClientError::new(ClientErrorKind::SocketClosed));
                    }
                };

                let mut write_state = current.lock().unwrap();

                send_connect_status(&app_handle, "Upgrading", "Sampling time difference.")?;

                return match active_connection.upgrade_direct() {
                    Ok(connection) => {
                        let mut write_state = current.lock().unwrap();
                        send_connect_status(&app_handle, "Connected successfully", "")?;
                        let _port = connection.get_port();

                        let (writer, reader) = connection.accept();
                        let client = Client::new(
                            app_handle.clone(),
                            reader,
                            writer,
                            self_port,
                        );

                        *write_state = Current::ConnectedTcp(client);
                        send_connected(&app_handle, Protocol::TCP)?;
                        Ok(())
                    }
                    Err(err) => {
                        let (old_connection, err) = err.split();

                        println!("{}", err);


                        let (writer, reader) = match old_connection.transform_to_slide() {
                            Ok(wr) => wr,
                            Err(_) => {
                                *write_state = Current::Broken;
                                send_connect_error(&app_handle, "Failed to connect.", "Could not establish sliding window.")?;
                                return Err(ClientError::new(ClientErrorKind::SocketClosed));
                            }
                        };

                        let client = Client::new(
                            app_handle.clone(),
                            reader,
                            writer,
                            self_port,
                        );

                        *write_state = Current::ConnectedUdp(client);
                        send_connected(&app_handle, Protocol::UDP)?;
                        Ok(())
                    }
                };
            }
            Err(err) => {
                let (old_c, err) = err.split();
                connection = old_c;

                match err.downcast_ref::<p2p::error::Error>() {
                    Some(err) => match err.kind() {
                        ErrorKind::CannotConnectToSelf => {
                            send_connect_error(&app_handle, "Cannot connect to self", "")?;
                            break;
                        }
                        _ => continue,
                    },
                    None => {}
                }
            }
        }
    }

    let mut write_state = current.lock().unwrap();

    *write_state = Current::Disconnected(connection);

    return Err(ClientError::new(ClientErrorKind::SocketClosed));
}
