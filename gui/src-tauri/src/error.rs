use std::error::Error;
use std::fmt::{Display, Formatter};



use std::sync::{PoisonError};
use std::sync::mpsc::SendError;
use std::{io};

use serde::{Serialize, Serializer};









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
    DataCorruptionError,
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

impl From<io::Error> for ClientError {
    fn from(_value: io::Error) -> Self {
        ClientError::new(ClientErrorKind::IOError)
    }
}

impl Serialize for ClientError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        serializer.serialize_str(format!("{:?}", self.kind).as_str())
    }
}