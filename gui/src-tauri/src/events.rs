use serde::Serialize;
use tauri::{AppHandle, Manager, Wry};

use crate::error::ClientError;

#[derive(Clone, serde::Serialize)]
struct Status {
    status: String,
    description: String,
    error: bool,
}


pub fn send_connect_status(handle: &AppHandle<Wry>, status: &str, description: &str) -> Result<(), ClientError> {
    handle
        .emit_all("app://update-status", Status { status: status.into(), description: description.into(), error: false })
        ?;

    Ok(())
}

pub fn send_bind_port(handle: &AppHandle<Wry>, port: u16) -> Result<(), ClientError> {
    handle
        .emit_all("app://update-port", port)
        ?;

    Ok(())
}

pub fn send_connect_error(handle: &AppHandle<Wry>, status: &str, description: &str) -> Result<(), ClientError> {
    handle
        .emit_all("app://update-status", Status { status: status.into(), description: description.into(), error: true })
        ?;

    Ok(())
}

pub fn send_init_error(handle: &AppHandle<Wry>, status: &str, description: &str) -> Result<(), ClientError> {
    handle
        .emit_all("app://socket-failed", Status { status: status.into(), description: description.into(), error: true })
        ?;

    Ok(())
}

#[derive(Clone, Copy, Serialize)]
pub enum Protocol {
    TCP,
    UDP
}

pub fn send_connected(handle: &AppHandle<Wry>, protocol: Protocol) -> Result<(), ClientError> {
    handle
        .emit_all("app://connected", protocol)
        ?;

    Ok(())
}

pub fn send_disconnect(handle: &AppHandle<Wry>) -> Result<(), ClientError> {
    handle
        .emit_all("app://disconnected", ())
        ?;

    Ok(())
}

pub fn send_offer(handle: &AppHandle<Wry>, name: String, hash: String, size: u64)  -> Result<(), ClientError> {
    handle
        .emit_all("app://new-offer", (name, hash, size))
        ?;

    Ok(())
}