use std::array::TryFromSliceError;
use std::fmt::{Debug, Display, Formatter};
use std::io;
use std::sync::mpsc::{RecvError, RecvTimeoutError, TryRecvError};
use std::time::{SystemTimeError};

#[derive(Debug)]
pub enum ErrorKind {
    TimedOut,
    SystemTimeError,
    Other,
    StateChangeFailed,
    CommunicationFailed,
    EncryptionFailed,
    ChannelFailed,
    IllegalByteStream,
    UndefinedRole
}

#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    source: Option<Box<dyn std::error::Error>>,
}

impl Error {

    pub fn new(kind: ErrorKind) -> Error {
        Error {kind, source: None }
    }

    pub fn kind(&self) -> &ErrorKind{
        &self.kind
    }

    pub fn to_kind(self) -> ErrorKind {self.kind}
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.source.as_ref() {
            None => {write!(f, "p2p Error of kind {:?} occurred.", self.kind)}
            Some(src) => {write!(f, "p2p Error of kind {:?} occurred. Source: {}", self.kind, src)}
        }
    }
}

impl std::error::Error for Error {

    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self.source.as_ref() {
            None => None,
            Some(b) => Some(b.as_ref())
        }
    }

}

impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Error {source: Some(Box::new(value)), kind: ErrorKind::CommunicationFailed}
    }
}

impl<C: 'static> From<ChangeStateError<C>> for Error {
    fn from(value: ChangeStateError<C>) -> Self {
        Error {source: Some(Box::new(value)), kind: ErrorKind::StateChangeFailed}
    }
}

impl From<SystemTimeError> for Error {
    fn from(value: SystemTimeError) -> Self {
        Error {source: Some(Box::new(value)), kind: ErrorKind::SystemTimeError}
    }
}

impl From<dryoc::Error> for Error {
    fn from(value: dryoc::Error) -> Self {
        Error {source: Some(Box::new(value)), kind: ErrorKind::EncryptionFailed}
    }
}

impl From<TryRecvError> for Error {
    fn from(value: TryRecvError) -> Self {
        Error {source: Some(Box::new(value)), kind: ErrorKind::ChannelFailed}
    }
}

impl From<RecvError> for Error {
    fn from(value: RecvError) -> Self {
        Error {source: Some(Box::new(value)), kind: ErrorKind::ChannelFailed}
    }
}

impl From<RecvTimeoutError> for Error {
    fn from(value: RecvTimeoutError) -> Self {
        Error {source: Some(Box::new(value)), kind: ErrorKind::TimedOut}
    }
}

impl From<TryFromSliceError> for Error {
    fn from(value: TryFromSliceError) -> Self {
        Error {source: Some(Box::new(value)), kind: ErrorKind::IllegalByteStream}
    }
}

pub struct ChangeStateError<C>(C, Box<dyn std::error::Error>);

impl<C> Debug for ChangeStateError<C> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Changing state failed with Error: {}", self.1)
    }
}

impl<C> Display for ChangeStateError<C> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Changing state failed with Error: {}", self.1)
    }
}

impl<C> std::error::Error for ChangeStateError<C> {}

impl<C> ChangeStateError<C> {
    pub fn new(state: C, err: Box<dyn std::error::Error>) -> ChangeStateError<C> {
        ChangeStateError(state, err)
    }

    pub fn to_state(self) -> C {
        self.0
    }

    pub fn to_err(self) -> Box<dyn std::error::Error> {
        self.1
    }

    pub fn as_state(&mut self) -> &mut C {
        &mut self.0
    }

    pub fn as_err(&mut self) -> &mut Box<dyn std::error::Error> {
        &mut self.1
    }

    pub fn split(self) -> (C, Box<dyn std::error::Error>) {
        (self.0, self.1)
    }
}