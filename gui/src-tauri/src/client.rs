use std::sync::{Arc, mpsc, Mutex, RwLock};
use std::sync::mpsc::Sender;
use std::thread;
use std::thread::{JoinHandle, sleep};
use std::time::Duration;

use tauri::{AppHandle, Wry};

use chunk::file::file::{create_data_vec, write_data_vec};
use chunk::general::general::{create_stop, get_chunk_count, read_send_header, read_stop, separate_header, validate_file};
use chunk::offer::offer::{create_offer_byte_msg, read_offer_vec};
use chunk::order::order::{create_order_byte_vec, read_order};
use p2p::client::{ClientReader, ClientWriter};
use p2p::error::ErrorKind;

use crate::error::{ClientError, ClientErrorKind};
use crate::events::{FileState, send_disconnect, send_file_state};

#[derive(Clone)]
pub struct File {
    pub(crate) hash: String,
    pub(crate) path: String,
    pub(crate) size: u64,
    pub(crate) name: String,
    cache: Vec<u8>,
}

impl File {
    fn new(hash: String, path: String, name: String, size: u64, cache: Vec<u8>) -> Self {
        File { hash, path, name, size, cache }
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
    write_command: Sender<WriteCommand>,
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

        let reader_thread = thread::spawn(move || {
            let app_handle_clone_3 = app_handle_clone_1.clone();
            let read = read_thread(drop_threads_clone_1, reader_clone, app_handle_clone_1, timeout, read_command_receiver, write_command_clone);
            match read {
                Ok(_) => println!("[CLIENT]: Read thread exited successfully"),
                Err(e) => {
                    println!("[CLIENT]: Read thread exited with error {}", e);
                    send_disconnect(&app_handle_clone_3).map_err(|_| ClientError::new(ClientErrorKind::MpscSendError))?;
                }
            };
            Ok(())
    });
    let writer_thread = thread::spawn( move | | {
        let app_handle_clone_3 = app_handle_clone_2.clone();
        let write = write_thread(drop_threads_clone_2, app_handle_clone_2, writer_clone, write_command_receiver);
        match write {
            Ok(_) => println!("[CLIENT]: Write thread exited successfully"),
            Err(e) => {
                println!("[CLIENT]: Write thread exited with error {}", e);
                send_disconnect(&app_handle_clone_3).map_err(|_| ClientError::new(ClientErrorKind::MpscSendError))?;
            }
        };
        Ok(())
    });

    Client { read_command, write_command, app_handle, reader, writer, drop_threads, port, reader_thread, writer_thread
}
}

pub fn get_port(&self) -> u16 {
    self.port
}

pub fn offer_file(&mut self, path: String) -> Result<(), ClientError> {
    let (file, file_name, file_size) = chunk::general::general::get_file_data(&path)?;
    let file_hash = chunk::hash::hash::get_hash_from_file(&file)?;

    let new_file = File::new(file_hash, path, file_name, file_size, Vec::new());

    send_file_state(&self.app_handle, new_file.clone(), FileState::Pending, 0.0, true)?;

    self.write_command.send(WriteCommand::Offer(new_file))?;
    Ok(())
}

pub fn accept_file(&mut self, hash: String, path: String) -> Result<(), ClientError> {
    let file = File::new(hash, path, "".to_string(), 0, Vec::new());

    self.read_command.send(ReadCommand::Receive(file))?;
    Ok(())
}

pub fn deny_file(&mut self, hash: String) -> Result<(), ClientError> {
    self.read_command.send(ReadCommand::Stop(hash))?;
    Ok(())
}

pub fn stop_file(&mut self, hash: String) -> Result<(), ClientError> {
    self.write_command.send(WriteCommand::StopSend(hash))?;
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
    Stop(String),
    // Sends stop command to other client
    StopSend(String),
    // Stops self sending
    Send(String, u64, u64), // HASH + START CHUNK + END CHUNK
}


fn read_thread<R: ClientReader>(dropper: Arc<RwLock<bool>>,
                                reader: Arc<Mutex<R>>,
                                app_handle: AppHandle<Wry>,
                                timeout: Option<Duration>,
                                command_receiver: mpsc::Receiver<ReadCommand>,
                                command_sender: Sender<WriteCommand>) -> Result<(), ClientError> {
    let mut reader = reader.lock()?;
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
                        None => {
                            println!("[READER] COMMAND : receive not found {}", file.hash);
                        }
                        Some(index) => {
                            let mut new_file = pending_files.swap_remove(index);
                            new_file.path = file.path;

                            let active_file = ActiveFile::from_file(new_file);
                            send_file_state(&app_handle, active_file.file.clone(), FileState::Transferring, 0.0, false)?;
                            active_files.push(active_file.clone());
                            command_sender.send(WriteCommand::Request(active_file))?;
                        }
                    }
                }
                ReadCommand::Pause(hash) => {
                    match active_files.iter().position(|wf| wf.file.hash == hash) {
                        None => {
                            println!("[READER] COMMAND : pause not found {}", hash);
                        }
                        Some(index) => {
                            let file = active_files.swap_remove(index);
                            let hash = file.file.hash.clone();
                            command_sender.send(WriteCommand::Stop(hash))?;
                            paused_files.push(file);
                        }
                    }
                }
                ReadCommand::Resume(hash) => {
                    match paused_files.iter().position(|wf| wf.file.hash == hash) {
                        None => {
                            println!("[READER] COMMAND : resume not found {}", hash);
                        }
                        Some(index) => {
                            let file = paused_files.swap_remove(index);
                            command_sender.send(WriteCommand::Request(file.clone()))?;
                            active_files.push(file);
                        }
                    }
                }
                ReadCommand::Stop(hash) => {
                    match active_files.iter().position(|wf| wf.file.hash == hash) {
                        None => {
                            println!("[READER] COMMAND : stop not found {}", hash);
                        }
                        Some(index) => {
                            send_file_state(&app_handle, active_files[index].file.clone(), FileState::Stopped, 0.0, false)?;
                            active_files.swap_remove(index);
                        }
                    }
                    command_sender.send(WriteCommand::Stop(hash))?;
                }
            },
            Err(_) => {}
        }

        let mut msg = match reader.read(timeout) {
            Ok(msg) => msg,
            Err(_err) => {
                match _err.kind() {
                    ErrorKind::TimedOut => continue,
                    _ => {
                        println!("[READER] : error reading {}", _err);
                        return Err(ClientError::new(ClientErrorKind::SocketClosed));
                    }
                }
            }
        };

        //println!("[READER] : msg {}", msg[0]);

        match msg[0] {
            0x02 => {//request file
                let order = read_order(&mut msg).map_err(|_err| {
                    println!("[READER] : error reading order {}", _err);
                    ClientError::new(ClientErrorKind::DataCorruptionError)
                })?;
                println!("[READER] : request {}", order.file_hash);

                command_sender.send(WriteCommand::Send(order.file_hash, order.start_num, order.end_num))?;
            }
            0x01 => {//offer file
                let offer = read_offer_vec(&msg).map_err(|_| ClientError::new(ClientErrorKind::DataCorruptionError))?;

                println!("[READER] : offer {}", offer.file_hash);

                let file = File::new(offer.file_hash, "".to_string(), offer.name, offer.size, msg);
                pending_files.push(file.clone());

                //send_offer(&app_handle, file.path, file.hash, file.size)?;
                send_file_state(&app_handle, file, FileState::Pending, 0.0, false)?;
            }
            0x03 => {//stop send file
                let hash = read_stop(&msg).map_err(|_| ClientError::new(ClientErrorKind::DataCorruptionError))?;

                println!("[READER] : stop {}", hash);

                command_sender.send(WriteCommand::StopSend(hash))?;
            }
            0x00 => { //file data
                let (header_vector, data_vector) = separate_header(&msg).map_err(|_| ClientError::new(ClientErrorKind::DataCorruptionError))?;
                let header_data = read_send_header(&header_vector).map_err(|_| ClientError::new(ClientErrorKind::DataCorruptionError))?;

                match active_files.iter().position(|wf| wf.file.hash == header_data.file_hash) {
                    None => {
                        println!("[READER] : unknown file {}", header_data.file_hash);
                    }
                    Some(index) => {
                        let mut file = &mut active_files[index];

                        println!("[READER] : file {}", header_data.file_hash);

                        // send file status to front end
                        let percent = file.current as f32 / file.stop as f32;
                        send_file_state(&app_handle, file.file.clone(), FileState::Transferring, percent, false)?;

                        let log_path = write_data_vec(&header_data, &data_vector, &file.file.path)?;

                        let act_num = header_data.chunk_pos;

                        file.current = act_num;

                        if file.current == file.stop {
                            match validate_file(&log_path, &file.file.hash) {
                                Ok((start, end)) => {
                                    if start == end && start == 0 {
                                        send_file_state(&app_handle, file.file.clone(), FileState::Completed, 1.0, false)?;
                                        active_files.remove(index);
                                    }
                                },
                                Err(_err) => {
                                    send_file_state(&app_handle, file.file.clone(), FileState::Corrupted, 1.0, false)?;
                                }
                            };
                        }
                    }
                }
            }
            _x => {// illegal opcode
                println!("[READER] : unknown opcode {}", _x);
            }
        }
    }
}

#[derive(Clone)]
struct ActiveFile {
    file: File,
    start: u64,
    stop: u64,
    current: u64,
}

impl ActiveFile {
    //TODO @Simon anhand der file und groesse anzahl der chunks berechnen und entsprechend anfuegen
    fn from_file(file: File) -> Self {
        let stop = get_chunk_count(file.size);
        Self { file, start: 1, stop, current: 1 }
    }
}

fn write_thread<W: ClientWriter>(dropper: Arc<RwLock<bool>>,
                                 app_handle: AppHandle<Wry>,
                                 writer: Arc<Mutex<W>>,
                                 command_receiver: mpsc::Receiver<WriteCommand>) -> Result<(), ClientError> {
    let mut writer = writer.lock()?;
    let mut files = Vec::<ActiveFile>::new();
    let mut offers = Vec::<File>::new();

    loop {
        {
            if *dropper.read()? {
                println!("dropper is true");
                return Ok(());
            }
        }

        match command_receiver.try_recv() {
            Ok(c) =>
                match c {
                    WriteCommand::Request(file) => {
                        let vec = create_order_byte_vec(file.start, file.stop, &file.file.hash)?;
                        println!("[WRITER] SENT: request {}", file.file.hash);
                        writer.write(&vec)?;
                    }
                    WriteCommand::Offer(file) => {
                        println!("[WRITER] SENT: offer {}", file.hash);
                        let vec = create_offer_byte_msg(&file.hash, file.size, &file.path)?;
                        offers.push(file);
                        writer.write(&vec)?;
                    }
                    WriteCommand::StopSend(hash) => {
                        match files.iter().position(|wf| wf.file.hash == hash) {
                            None => {
                                println!("[WRITER]   OP: stop send unknown {}", hash);
                            }
                            Some(index) => {
                                println!("[WRITER]   OP: stop send {}", hash);
                                files.swap_remove(index);
                            }
                        }
                    }
                    WriteCommand::Send(hash, start, stop) => {
                        match offers.iter().position(|of| of.hash == hash) {
                            None => {
                                println!("[WRITER]   OP: send unknown {}", hash);
                            }
                            Some(index) => {
                                println!("[WRITER]   OP: send {} with {} : {}", hash, start, stop);
                                if stop != 0 {
                                    let file = offers.swap_remove(index);
                                    send_file_state(&app_handle, file.clone(), FileState::Transferring, 0.0, true)?;
                                    let active_file = ActiveFile { file, stop, start, current: 0 };
                                    files.push(active_file);
                                }
                            }
                        }
                    }
                    WriteCommand::Stop(hash) => {
                        let vec = create_stop(&hash)?;
                        println!("[WRITER] SENT: stop {}", hash);
                        writer.write(&vec)?;
                    }
                },
            Err(_) => {}
        };


        for i in 0..files.len() {
            if i >= files.len() {
                break;
            }

            let mut file = &mut files[i];

            if file.current < 1 {
                file.current = 1;
            }

            let data_vec = create_data_vec(&file.file.path, file.current, &file.file.hash).map_err(|_| ClientError::new(ClientErrorKind::IOError))?;

            match writer.write(&data_vec) {
                Ok(_) => {
                    println!("[WRITER] SENT: data {}", file.file.hash);
                    let percent = file.current as f32 / file.stop as f32;
                    send_file_state(&app_handle, file.file.clone(), FileState::Transferring, percent, true)?;
                }
                Err(_err) => {
                    send_disconnect(&app_handle)?;
                    return Err(ClientError::new(ClientErrorKind::SocketClosed));
                }
            };

            if file.current == file.stop{
                send_file_state(&app_handle, file.file.clone(), FileState::Completed, 1.0, true)?;
                files.remove(i);
            }else {
                file.current += 1;
            }
        }

        if files.len() == 0 {
            sleep(Duration::from_millis(100));
        }
    }
}