use tauri::{AppHandle, Manager, Wry};

use crate::error::ClientError;

#[derive(Clone, serde::Serialize)]
struct Status {
    status: String,
    description: String,
    error: bool,
}


fn send_bind_port(handle: &AppHandle<Wry>, port: u16) -> Result<(), ClientError> {
    handle
        .emit_all("app://update-port", port)
        ?;

    Ok(())
}

fn send_connect_status(handle: &AppHandle<Wry>, status: &str, description: &str) -> Result<(), ClientError> {
    handle
        .emit_all("app://update-status", Status { status: status.into(), description: description.into(), error: false })
        ?;

    Ok(())
}

fn send_connect_error(handle: &AppHandle<Wry>, status: &str, description: &str) -> Result<(), ClientError> {
    handle
        .emit_all("app://update-status", Status { status: status.into(), description: description.into(), error: true })
        ?;

    Ok(())
}

fn send_init_error(handle: &AppHandle<Wry>, status: &str, description: &str) -> Result<(), ClientError> {
    handle
        .emit_all("app://socket-failed", Status { status: status.into(), description: description.into(), error: true })
        ?;

    Ok(())
}