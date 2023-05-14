use serde::Serialize;
use tauri::{AppHandle, Manager, Wry};
use crate::client::File;

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

#[derive(Serialize, Clone)]
pub enum FileState{
    Transferring,
    Pending,
    Completed,
    Aborted,
    Stopped,
    Corrupted
}

#[derive(Serialize, Clone)]
pub struct FileJson {
    name: String,
    path: String,
    size: u64,
    hash: String,
    percent: f32,
    state: FileState,
    is_sender: bool
}

pub fn send_file_state(handle: &AppHandle<Wry>, file: File, file_state: FileState, percent: f32, is_sender: bool) -> Result<(), ClientError> {
    let payload = FileJson {
        name: file.name, //TODO
        path: file.path,
        size: file.size,
        hash: file.hash,
        percent,
        state: file_state,
        is_sender
    };

    handle
        .emit_all("app://file-update", payload)
        ?;

    Ok(())
}