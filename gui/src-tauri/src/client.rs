use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::read;
use std::marker::PhantomData;
use std::mem::size_of;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, mpsc, Mutex, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::thread;
use std::thread::{JoinHandle, sleep};
use std::time::Duration;

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
    reader: Arc<Mutex<R>>,
    writer: Arc<Mutex<W>>,
    drop_threads: Arc<RwLock<bool>>,
    port: u16,
    reader_thread: JoinHandle<Result<(), ClientError>>,
    writer_thread: JoinHandle<Result<(), ClientError>>,
}

impl<W: ClientWriter + Send + 'static, R: ClientReader + Send + 'static> Client<W, R> {
    pub fn new(reader: R, writer: W, timeout: Option<Duration>, port: u16) -> Self {
        let drop_threads = Arc::new(RwLock::new(false));
        let reader = Arc::new(Mutex::new(reader));
        let writer = Arc::new(Mutex::new(writer));

        let reader_clone = reader.clone();
        let writer_clone = writer.clone();
        let drop_threads_clone_1 = drop_threads.clone();
        let drop_threads_clone_2 = drop_threads.clone();

        let reader_thread = thread::spawn(move || read_thread(drop_threads_clone_1, reader_clone, timeout));
        let writer_thread = thread::spawn(move || write_thread(drop_threads_clone_2, writer_clone));

        Client {reader, writer, drop_threads, port, reader_thread, writer_thread}
    }

    pub fn get_port(&self) -> u16 {
        self.port
    }
}

impl<W: ClientWriter + Send, R: ClientReader + Send> Drop for Client<W, R> {
    fn drop(&mut self) {
        //should panic if this fails
        let mut dropper = self.drop_threads.write().unwrap();
        *dropper = true;
    }
}


fn read_thread<R: ClientReader>(dropper: Arc<RwLock<bool>>, reader: Arc<Mutex<R>>, timeout: Option<Duration>) -> Result<(), ClientError> {
    // TODO remove unwrap
    let mut reader = reader.lock().unwrap();

    loop {
        {
            if *dropper.read()? {
                return Ok(());
            }
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

fn write_thread<W: ClientWriter>(dropper: Arc<RwLock<bool>>, writer: Arc<Mutex<W>>) -> Result<(), ClientError> {
    // TODO remove unwrap
    let mut writer = writer.lock().unwrap();

    loop {
        {
            if *dropper.read()? {
                return Ok(());
            }
        }

        sleep(Duration::from_secs(1));
    }
}