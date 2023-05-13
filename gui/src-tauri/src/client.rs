use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::read;
use std::marker::PhantomData;
use std::mem::size_of;
use std::ops::{Deref, DerefMut, Index};
use std::sync::{Arc, mpsc, Mutex, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::sync::mpsc::{RecvTimeoutError, Sender, TryRecvError};
use std::thread;
use std::thread::{JoinHandle, sleep};
use std::time::Duration;

use tauri::{AppHandle, Wry};

use p2p::client::{ClientReader, ClientWriter, EncryptedReader, EncryptedWriter};
use p2p::client::udp::UdpClientReader;
use p2p::error::ErrorKind;

use crate::error::{ClientError, ClientErrorKind};
use crate::events::{send_disconnect, send_offer};

#[derive(Clone)]
pub struct File {
    hash: String,
    path: String,
    size: u64,
    cache: Vec<u8>,
}

impl File {
    fn new(hash: String, path: String, size: u64, cache: Vec<u8>) -> Self {
        File { hash, path, size, cache }
    }
}

pub struct Client<W: ClientWriter + Send, R: ClientReader + Send> {
    app_handle: AppHandle<Wry>,
    reader: Arc<Mutex<R>>,
    writer: Arc<Mutex<W>>,
    drop_threads: Arc<RwLock<bool>>,
    port: u16,
    reader_thread: JoinHandle<Result<(), ClientError>>,
    writer_thread: JoinHandle<Result<(), ClientError>>,
    read_command: Sender<ReadCommand>,
    write_command: Sender<WriteCommand>
}

impl<W: ClientWriter + Send + 'static, R: ClientReader + Send + 'static> Client<W, R> {
    pub fn new(app_handle: AppHandle<Wry>, reader: R, writer: W, timeout: Option<Duration>, port: u16) -> Self {
        let drop_threads = Arc::new(RwLock::new(false));
        let reader = Arc::new(Mutex::new(reader));
        let writer = Arc::new(Mutex::new(writer));

        let reader_clone = reader.clone();
        let writer_clone = writer.clone();
        let drop_threads_clone_1 = drop_threads.clone();
        let drop_threads_clone_2 = drop_threads.clone();
        let app_handle_clone_1 = app_handle.clone();
        let app_handle_clone_2 = app_handle.clone();

        let (read_command, read_command_receiver) = mpsc::channel::<ReadCommand>();
        let (write_command, write_command_receiver) = mpsc::channel::<WriteCommand>();

        let write_command_clone = write_command.clone();

        let reader_thread = thread::spawn(move || read_thread(drop_threads_clone_1, reader_clone, app_handle_clone_1, timeout, read_command_receiver, write_command_clone));
        let writer_thread = thread::spawn(move || write_thread(drop_threads_clone_2, app_handle_clone_2, writer_clone, write_command_receiver));

        Client { read_command, write_command, app_handle, reader, writer, drop_threads, port, reader_thread, writer_thread }
    }

    pub fn get_port(&self) -> u16 {
        self.port
    }

    //TODO @Simon notwendige Logik implementieren
    pub fn offer_file(&mut self, path: String) -> Result<(), ClientError> {
        // hash erstellen größe berechnen - wenn file nicht existiert entsprechend client error returnen

        //let (file, file_name, file_size) = chunk::file::get_file_data(&path);


        let new_file = File::new("HASH".into(), path, 100, Vec::new());

        self.write_command.send(WriteCommand::Offer(new_file))?;
        Ok(())
    }

    pub fn accept_file(&mut self, hash: String, path: String) -> Result<(), ClientError> {
        let file = File::new(hash, path, 0, Vec::new());

        self.read_command.send(ReadCommand::Receive(file))?;
        Ok(())
    }

    pub fn deny_file(&mut self, hash: String) -> Result<(), ClientError> {
        self.read_command.send(ReadCommand::Stop(hash))?;
        Ok(())
    }

    pub fn pause_file(&mut self, hash: String) -> Result<(), ClientError> {
        self.read_command.send(ReadCommand::Pause(hash))?;
        Ok(())
    }
}

impl<W: ClientWriter + Send, R: ClientReader + Send> Drop for Client<W, R> {
    fn drop(&mut self) {
        //should panic if this fails
        let mut dropper = self.drop_threads.write().unwrap();
        *dropper = true;
    }
}

enum ReadCommand {
    Receive(File),
    Pause(String),
    Resume(String),
    Stop(String),
}

enum WriteCommand {
    Request(ActiveFile),
    Offer(File),
    Stop(String), // Sends stop command to other client
    StopSend(String), // Stops self sending
    Send(String, u64, u64) // HASH + START CHUNK + END CHUNK
}


fn read_thread<R: ClientReader>(dropper: Arc<RwLock<bool>>,
                                reader: Arc<Mutex<R>>,
                                app_handle: AppHandle<Wry>,
                                timeout: Option<Duration>,
                                command_receiver: mpsc::Receiver<ReadCommand>,
                                command_sender: Sender<WriteCommand>) -> Result<(), ClientError> {
    // TODO remove unwrap
    let mut reader = reader.lock().unwrap();
    let mut paused_files: Vec<ActiveFile> = vec![];
    let mut active_files: Vec<ActiveFile> = vec![];
    let mut pending_files: Vec<File> = vec![];
    loop {
        {
            if *dropper.read()? {
                return Ok(());
            }
        }

        match command_receiver.try_recv() {
            Ok(c) => match c {
                ReadCommand::Receive(file) => {
                    match pending_files.iter().position(|wf| wf.hash == file.hash) {
                        None => {}
                        Some(index) => {
                            let mut new_file = pending_files.swap_remove(index);
                            new_file.path = file.path;

                            let active_file = ActiveFile::from_file(new_file);
                            active_files.push(active_file.clone());
                            command_sender.send(WriteCommand::Request(active_file))?;
                        }
                    }
                }
                ReadCommand::Pause(hash) => {
                    match active_files.iter().position(|wf| wf.file.hash == hash) {
                        None => {}
                        Some(index) => {
                            let file = active_files.swap_remove(index);
                            let hash = file.file.hash.clone();
                            command_sender.send(WriteCommand::Stop(hash))?;
                            paused_files.push(file);
                        },
                    }
                }
                ReadCommand::Resume(hash) => {
                    match paused_files.iter().position(|wf| wf.file.hash == hash) {
                        None => {}
                        Some(index) => {
                            let file = paused_files.swap_remove(index);
                            command_sender.send(WriteCommand::Request(file.clone()))?;
                            active_files.push(file);
                        },
                    }
                }
                ReadCommand::Stop(hash) => {
                    match active_files.iter().position(|wf| wf.file.hash == hash) {
                        None => {}
                        Some(index) => {active_files.swap_remove(index);},
                    }
                }
            },
            Err(_) => {}
        }

        let msg = match reader.read(timeout) {
            Ok(msg) => msg,
            Err(err) => {
                match err.kind() {
                    ErrorKind::TimedOut => { continue; }
                    _ => {
                        send_disconnect(&app_handle)?;
                        return Err(ClientError::new(ClientErrorKind::SocketClosed));
                    }
                }
            }
        };

        //TODO @Simon handle message
        match msg[0] {
            0x0A => {//request file
                println!("(recv) : request");
                //command_sender.send(WriteCommand::Send("FILE HASH".into(), 0, 100))?; - Tell sender to start sending the file
            }
            0xAB => {//offer file
                println!("(recv) : offer");
                // pending_files.push(); - file muss in pending liste gepusht werden
                // send_offer(&app_handle, "NAME".into(), "HASH".into(), 100)?; - file im frontend anzeigen
            }
            0xBB => {//stop send file
                println!("(recv) : stop");
                // command_sender.send(WriteCommand::StopSend("HAHS")) - tell command sender to stop sending
            }
            0x11 => {
                println!("(recv) : package");
            } // file package bitte cachen und wenn cache groß genug > 20 zb dann auf festplatte schreiben
            _ => {} // illegal opcode
        }
    }
}

#[derive(Clone)]
struct ActiveFile {
    file: File,
    start: u64,
    stop: u64,
    current: u64
}

impl ActiveFile {
    //TODO @Simon anhand der file und groesse anzahl der chunks berechnen und entsprechend anfuegen
    fn from_file(file: File) -> Self {
        todo!()
    }
}

fn write_thread<W: ClientWriter>(dropper: Arc<RwLock<bool>>,
                                 app_handle: AppHandle<Wry>,
                                 writer: Arc<Mutex<W>>,
                                 command_receiver: mpsc::Receiver<WriteCommand>) -> Result<(), ClientError> {
    // TODO remove unwrap
    let mut writer = writer.lock().unwrap();
    let mut files = Vec::<ActiveFile>::new();
    let mut offers = Vec::<File>::new();

    loop {
        {
            if *dropper.read()? {
                return Ok(());
            }
        }

        match command_receiver.try_recv() {
            Ok(c) =>
                match c {
                    WriteCommand::Request(file) => {
                        //TODO SEND REQUEST WITH GIVEN CHUNKS

                        match writer.write(&[0x0A]) {
                            Ok(_) => {
                                println!("(send) : request");
                            }
                            Err(_err) => {
                               println!("disconnect");
                               send_disconnect(&app_handle)?;
                               return Err(ClientError::new(ClientErrorKind::SocketClosed));
                            }
                        };
                    }
                    WriteCommand::Offer(file) => {
                        //TODO SEND OFFER

                        match writer.write(&[0xAB]) {
                            Ok(_) => {
                                println!("(send) : offer");
                            }
                            Err(_err) => {
                                println!("disconnect");
                                send_disconnect(&app_handle)?;
                                return Err(ClientError::new(ClientErrorKind::SocketClosed));
                            }
                        };
                    }
                    WriteCommand::StopSend(hash) => {
                        match files.iter().position(|wf| wf.file.hash == hash) {
                            None => {}
                            Some(index) => {files.swap_remove(index);},
                        }
                    }
                    WriteCommand::Send(hash, start, stop) => {
                        match offers.iter().position(|of| of.hash == hash) {
                            None => {}
                            Some(index) => {
                                let file = offers.swap_remove(index);
                                let active_file = ActiveFile {file, stop, start, current: 0};
                            }
                        }
                    }
                    WriteCommand::Stop(hash) => {
                        // hash zu msg hinzufuegen
                        match writer.write(&[0xBB]) {
                            Ok(_) => {
                                println!("(send) : stop");
                            }
                            Err(_err) => {
                                println!("disconnect");
                                send_disconnect(&app_handle)?;
                                return Err(ClientError::new(ClientErrorKind::SocketClosed));
                            }
                        };
                    }
                },
            Err(_) => {}
        };


        for file in &mut files {
            // TODO jeweils max 10mb aus dateisystem lesen und versenden
            // current ist die aktuelle positon (offset), start und stop sind die grenzen angegeben in chunks

            match writer.write(file.file.hash.as_bytes()) {
                Ok(_) => {
                    println!("(send) : package");
                }
                Err(err) => {
                    match err.kind() {
                        ErrorKind::TimedOut => { continue; }
                        _ => {
                            send_disconnect(&app_handle)?;
                            return Err(ClientError::new(ClientErrorKind::SocketClosed));
                        }
                    }
                }
            };
        }

        if files.len() == 0 {
            sleep(Duration::from_millis(100));
        }

    }
}