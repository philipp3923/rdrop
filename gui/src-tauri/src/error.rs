use std::error::Error;
use std::fmt::{Display, Formatter};
use std::marker::PhantomData;
use std::mem::size_of;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, mpsc, MutexGuard, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::sync::mpsc::SendError;
use std::thread;
use std::time::Duration;
use serde::{Serialize, Serializer};

use tauri::async_runtime::JoinHandle;
use tauri::InvokeError;

use p2p::client::{ClientReader, ClientWriter, EncryptedReader, EncryptedWriter};
use p2p::client::udp::UdpClientReader;
use p2p::error::ErrorKind;
use crate::handle;

#[derive(Debug)]
pub enum ClientErrorKind {
    LockPoisoned,
    SocketClosed,
    WrongState,
    MpscSendError,
    Ipv6ParseFailed,
    SendToFrontendFailed,
    NotFound,
    IOError,
    CommunicationError
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

impl From<tauri::Error> for ClientError {
    fn from(_value: tauri::Error) -> Self {
        ClientError::new(ClientErrorKind::SendToFrontendFailed)
    }
}

impl<T> From<SendError<T>> for ClientError {
    fn from(_value: SendError<T>) -> Self {
        ClientError::new(ClientErrorKind::MpscSendError)
    }
}

impl<T> From<PoisonError<T>> for ClientError {
    fn from(_value: PoisonError<T>) -> Self {
        ClientError::new(ClientErrorKind::LockPoisoned)
    }
}

impl From<p2p::error::Error> for ClientError {
    fn from(_value: p2p::error::Error) -> Self {
        ClientError::new(ClientErrorKind::CommunicationError)
    }
}

impl From<std::io::Error> for ClientError {
    fn from(_value: std::io::Error) -> Self {
        ClientError::new(ClientErrorKind::IOError)
    }
}

impl Serialize for ClientError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        serializer.serialize_str(format!("{:?}", self.kind).as_str())
    }
}