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

use crate::error::{ClientError, ClientErrorKind};

pub struct FileInfo {
    hash: String,
    path: String,
}

pub struct FileOffer {
    name: String,
    hash: String,
    size: u64,
}

pub struct FileOrder {
    name: String,
    hash: String,
    path: String,
    start_chunk: u32,
    end_chunk: u32,
    chunk_size: u32,
}

pub struct Client<W: ClientWriter + Send, R: ClientReader + Send> {
    reader: PhantomData<R>,
    writer: PhantomData<W>,
    drop_threads: Arc<RwLock<bool>>,
    reader_thread: JoinHandle<Result<(), ClientError>>,
    writer_thread: JoinHandle<Result<(), ClientError>>,
}

impl<W: ClientWriter + Send, R: ClientReader + Send> Client<W, R> {
    fn new(reader: R, writer: W, timeout: Option<Duration>) -> Self {
        let drop_threads = Arc::new(RwLock::new(false));

        //let reader_thread = thread::spawn(move || Client::read_thread(drop_threads.clone(), reader, timeout));

        todo!()
    }

    fn read_thread(dropper: Arc<RwLock<bool>>, mut reader: R, timeout: Option<Duration>) -> Result<(), ClientError> {
        let dropper_read = dropper.read()?;
        loop {
            if *dropper_read {
                return Ok(());
            }

            let msg = match reader.read(timeout) {
                Ok(_) => {}
                Err(err) => {
                    match err.kind() {
                        ErrorKind::TimedOut => { continue; }
                        _ => {
                            // send msg to frontend
                            return Err(ClientError::new(ClientErrorKind::SocketClosed));
                        }
                    }
                }
            };

            // handle message
        }
    }

    fn write_thread() {
        loop {}
    }
}

impl<W: ClientWriter + Send, R: ClientReader + Send> Drop for Client<W, R> {
    fn drop(&mut self) {
        //should panic if this fails
        let mut dropper = self.drop_threads.write().unwrap();
        *dropper = true;
        self.reader_thread.abort();
        self.writer_thread.abort();
    }
}