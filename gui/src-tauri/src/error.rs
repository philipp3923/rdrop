use std::error::Error;
use std::fmt::{Display, Formatter};
use std::marker::PhantomData;
use std::mem::size_of;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, mpsc, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::thread;
use std::time::Duration;

use tauri::async_runtime::JoinHandle;

use p2p::client::{ClientReader, ClientWriter, EncryptedReader, EncryptedWriter};
use p2p::client::udp::UdpClientReader;
use p2p::error::ErrorKind;

#[derive(Debug)]
pub enum ClientErrorKind {
    LockPoisoned,
    SocketClosed,
    WrongState,
    Ipv6ParseFailed,
    SendToFrontendFailed
}

#[derive(Debug)]
pub struct ClientError {
    kind: ClientErrorKind,
    source: Option<Box<dyn Error + Send + Sync>>,
}

impl ClientError {
    pub(crate) fn new(kind: ClientErrorKind) -> Self {
        ClientError { kind, source: None }
    }

    fn with_source(kind: ClientErrorKind, source: Box<dyn Error + Send + Sync>) -> Self {
        ClientError { kind, source: Some(source) }
    }
}

impl Error for ClientError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self.source.as_ref() {
            None => None,
            Some(b) => Some(b.as_ref())
        }
    }
}

impl Display for ClientError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.source.as_ref() {
            None => { write!(f, "ClientError of kind {:?} occurred.", self.kind) }
            Some(src) => { write!(f, "ClientError of kind {:?} occurred. Source: {}", self.kind, src) }
        }
    }
}

impl From<PoisonError<RwLockReadGuard<'_, bool>>> for ClientError {
    fn from(_value: PoisonError<RwLockReadGuard<'_, bool>>) -> Self {
        ClientError::new(ClientErrorKind::LockPoisoned)
    }
}

impl From<PoisonError<RwLockWriteGuard<'_, bool>>> for ClientError {
    fn from(_value: PoisonError<RwLockWriteGuard<'_, bool>>) -> Self {
        ClientError::new(ClientErrorKind::LockPoisoned)
    }
}

impl From<tauri::Error> for ClientError {
    fn from(_value: tauri::Error) -> Self {
        ClientError::new(ClientErrorKind::SendToFrontendFailed)
    }
}