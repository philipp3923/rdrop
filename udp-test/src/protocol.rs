use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::net::{Ipv6Addr, TcpStream};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use dryoc::dryocbox::{Bytes, KeyPair};
use dryoc::dryocstream::{DryocStream, Header, Pull, Push};
use dryoc::kx::{Session, SessionKey};
use dryoc::sign::PublicKey;
use rand::{Rng, thread_rng};
use socket2::Socket;

use crate::client::{ActiveClient, ClientReader, ClientWriter, EncryptedReader, EncryptedWriter, WaitingClient};
use crate::client::tcp::{TcpActiveClient, TcpClientReader, TcpClientWriter, TcpWaitingClient};
use crate::client::udp::{UdpActiveClient, UdpClientReader, UdpClientWriter, UdpWaitingClient};
use crate::protocol_old::connect;

pub trait EncryptionState {}

pub trait ConnectionState {}

pub trait ProtocolState {
    type Writer: ClientWriter;
    type Reader: ClientReader;
}

pub struct Encrypted<P: ProtocolState> {
    encrypted_reader: EncryptedReader<P::Reader>,
    encrypted_writer: EncryptedWriter<P::Writer>,
    clock_diff_samples: Vec<i128>,
    max_delay: u128,
}

pub struct Plain<P: ProtocolState> {
    plain_reader: P::Reader,
    plain_writer: P::Writer,
}

impl<P: ProtocolState> EncryptionState for Encrypted<P> {}

impl<P: ProtocolState> EncryptionState for Plain<P> {}

#[derive(PartialEq, Debug)]
enum Role {
    Server,
    Client,
    None,
}

pub struct Active<E: EncryptionState> {
    role: Role,
    timeout: Option<Duration>,
    client: E,
    peer_ip: Ipv6Addr,
}

pub struct Waiting {
    waiting_client: UdpWaitingClient,
}

impl<E: EncryptionState> ConnectionState for Active<E> {}

impl ConnectionState for Waiting {}

pub struct Udp {}

pub struct Tcp {}

impl ProtocolState for Udp {
    type Writer = UdpClientWriter;
    type Reader = UdpClientReader;
}

impl ProtocolState for Tcp {
    type Writer = TcpClientWriter;
    type Reader = TcpClientReader;
}

pub struct Connection<C: ConnectionState> {
    state: C,
}

impl Connection<Waiting> {
    pub fn new(port: Option<u16>) -> Result<Connection<Waiting>, Box<dyn Error>> {
        let waiting_client = UdpWaitingClient::new(port)?;
        let state = Waiting { waiting_client };
        Ok(Connection { state })
    }

    pub fn get_port(&self) -> u16 {
        self.state.waiting_client.get_port()
    }

    pub fn connect(
        self,
        peer: Ipv6Addr,
        port: u16,
        connect_timeout: Option<Duration>,
        disconnect_timeout: Option<Duration>,
    ) -> Result<Connection<Active<Plain<Udp>>>, ChangeStateError<Self>> {
        let udp_active_client = match self.state.waiting_client.connect(peer, port, connect_timeout, disconnect_timeout) {
            Ok(connection) => connection,
            Err(err) => {
                let err = err.split();
                return Err(ChangeStateError(Connection { state: Waiting { waiting_client: err.0 } }, err.1));
            }
        };

        Ok(Connection::<Active<Plain<Udp>>>::new(udp_active_client, disconnect_timeout, peer))
    }
}

impl Connection<Active<Plain<Udp>>> {
    fn new(udp_active_client: UdpActiveClient, timeout: Option<Duration>, peer_ip: Ipv6Addr) -> Connection<Active<Plain<Udp>>> {
        let (writer, reader) = udp_active_client.split();

        Connection { state: Active { peer_ip, timeout, role: Role::None, client: Plain { plain_reader: reader, plain_writer: writer } } }
    }

    pub fn encrypt(mut self) -> Result<Connection<Active<Encrypted<Udp>>>, ChangeStateError<Self>> {
        if self.state.role == Role::None {
            match self.negotiate_roles() {
                Ok(_) => {}
                Err(e) => { return Err(ChangeStateError::new(self, e)); }
            }
        }

        let (decrypt_key, encrypt_key) = match self.exchange_keys() {
            Ok(keys) => keys,
            Err(e) => { return Err(ChangeStateError::new(self, e)); }
        };

        let (pull_stream, push_stream) = match self.generate_crypto_streams(decrypt_key, encrypt_key) {
            Ok(streams) => streams,
            Err(e) => { return Err(ChangeStateError::new(self, e)); }
        };

        let encrypted_reader = EncryptedReader::new(pull_stream, self.state.client.plain_reader);
        let encrypted_writer = EncryptedWriter::new(push_stream, self.state.client.plain_writer);

        let connection = Connection::<Active<Encrypted<Udp>>> { state: Active { peer_ip: self.state.peer_ip, role: self.state.role, timeout: self.state.timeout, client: Encrypted { max_delay: 0, clock_diff_samples: Vec::new(), encrypted_writer, encrypted_reader } } };

        Ok(connection)
    }

    fn exchange_keys(&mut self) -> Result<(SessionKey, SessionKey), Box<dyn Error>> {
        assert_ne!(self.state.role, Role::None);

        let my_keypair = KeyPair::gen();

        self.state.client.plain_writer.write(my_keypair.public_key.as_slice())?;

        let peer_public_key = self.state.client.plain_reader.read(self.state.timeout)?;

        assert_eq!(peer_public_key.len(), 32);

        let peer_public_key: [u8; 32] = peer_public_key.as_slice().try_into()?;

        let peer_public_key = PublicKey::from(peer_public_key);

        // Role is either Server or Client. This is guaranteed by the assert_ne in the first line of this method
        let my_session_keys = match self.state.role {
            Role::Server => Session::new_server_with_defaults(&my_keypair, &peer_public_key)?,
            _ => Session::new_client_with_defaults(&my_keypair, &peer_public_key)?,
        };

        return Ok(my_session_keys.into_parts());
    }

    fn generate_crypto_streams(&mut self, decrypt_key: SessionKey, encrypt_key: SessionKey) -> Result<(DryocStream<Pull>, DryocStream<Push>), Box<dyn Error>> {
        let (push_stream, mut header): (_, Header) = DryocStream::init_push(&encrypt_key);

        self.state.client.plain_writer.write(header.as_slice())?;

        let header = self.state.client.plain_reader.read(self.state.timeout)?;

        let pull_stream = DryocStream::init_pull(&decrypt_key, &header);

        return Ok((pull_stream, push_stream));
    }

    fn negotiate_roles(&mut self) -> Result<(), Box<dyn Error>> {
        let mut rng = thread_rng();

        loop {
            let my_number: [u8; 2] = [rng.gen(), rng.gen()];
            self.state.client.plain_writer.write(my_number.as_slice())?;
            let peer_number = self.state.client.plain_reader.read(self.state.timeout)?;

            assert_eq!(peer_number.len(), 2);

            if my_number.as_slice() != peer_number.as_slice() {
                match my_number.as_slice() > peer_number.as_slice() {
                    true => self.state.role = Role::Server,
                    false => self.state.role = Role::Client,
                }

                break;
            }
        }

        Ok(())
    }
}

impl Connection<Active<Encrypted<Udp>>> {
    pub fn upgrade(mut self) -> Result<Connection<Active<Encrypted<Tcp>>>, ChangeStateError<Self>> {
        let mut tcp_client = match TcpWaitingClient::new(None) {
            Ok(client) => client,
            Err(err) => return Err(ChangeStateError::new(self, err))
        };

        let peer_port = match self.exchange_ports(tcp_client.get_port()) {
            Ok(p) => p,
            Err(err) => return Err(ChangeStateError::new(self, err))
        };

        let tcp_client = match self.multi_sample_and_connect(tcp_client, peer_port, 10) {
            Ok(client) => client,
            Err(client) => {
                match self.sample_and_connect(client, peer_port) {
                    Ok(c) => c,
                    Err(err) => return Err(ChangeStateError::new(self, err.1)),
                }
            }
        };


        let (tcp_writer, tcp_reader) = tcp_client.split();

        let encrypted_reader = EncryptedReader::new(self.state.client.encrypted_reader.pull_stream, tcp_reader);
        let encrypted_writer = EncryptedWriter::new(self.state.client.encrypted_writer.push_stream, tcp_writer);

        let connection = Connection::<Active<Encrypted<Tcp>>> { state: Active { peer_ip: self.state.peer_ip, role: self.state.role, timeout: self.state.timeout, client: Encrypted { encrypted_writer, clock_diff_samples: self.state.client.clock_diff_samples, encrypted_reader, max_delay: self.state.client.max_delay } } };

        return Ok(connection);
    }

    fn multi_sample_and_connect(&mut self, mut tcp_client: TcpWaitingClient, peer_port: u16, tries: u8) -> Result<TcpActiveClient, TcpWaitingClient> {
        for _ in 0..tries {
            tcp_client = match self.sample_and_connect(tcp_client, peer_port) {
                Ok(c) => return Ok(c),
                Err(err) => err.0,
            };
        }

        Err(tcp_client)
    }

    fn sample_and_connect(&mut self, tcp_client: TcpWaitingClient, peer_port: u16) -> Result<TcpActiveClient, ChangeStateError<TcpWaitingClient>> {
        let wait_time: Duration;

        match self.state.role {
            Role::Server => {
                match self.collect_samples(50) {
                    Ok(_) => {}
                    Err(err) => return Err(ChangeStateError::new(tcp_client, err)),
                };
                wait_time = match self.set_connect_time() {
                    Ok(t) => t,
                    Err(err) => return Err(ChangeStateError::new(tcp_client, err)),
                };
            }
            Role::Client => {
                match self.provide_samples() {
                    Ok(_) => {}
                    Err(err) => return Err(ChangeStateError::new(tcp_client, err)),
                };

                wait_time = match self.get_connect_time() {
                    Ok(t) => t,
                    Err(err) => return Err(ChangeStateError::new(tcp_client, err)),
                };
            }
            Role::None => todo!(),
        }

        return tcp_client.connect(self.state.peer_ip, peer_port, Some(wait_time));
    }

    fn set_connect_time(&mut self) -> Result<Duration, Box<dyn Error>> {
        self.state.client.clock_diff_samples.sort();
        let diffs = self.state.client.clock_diff_samples.as_mut_slice();

        let median_diff = if diffs.len() % 2 == 0 {
            (diffs[diffs.len() / 2] + diffs[diffs.len() / 2 - 1]) / 2
        } else {
            diffs[diffs.len() / 2]
        };

        let connect_time = (SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_nanos()
            + self.state.client.max_delay * 10) as i128;

        let connect_time_with_diff = (connect_time - median_diff) as u64;

        self.state.client.encrypted_writer.write(connect_time_with_diff.to_be_bytes().as_slice())?;

        let connect_delay_nanos = connect_time as u128 - SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();

        Ok(Duration::from_nanos(connect_delay_nanos as u64))
    }

    fn get_connect_time(&mut self) -> Result<Duration, Box<dyn Error>> {
        let nanos = self.state.client.encrypted_reader.read(self.state.timeout)?;
        let connect_time = nanos.try_into().unwrap();
        let connect_time = u64::from_be_bytes(connect_time);

        let connect_delay_nanos = connect_time as u128 - SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();

        Ok(Duration::from_nanos(connect_delay_nanos as u64))
    }

    fn exchange_ports(&mut self, port: u16) -> Result<u16, Box<dyn Error>> {
        self.state.client.encrypted_writer.write(&port.to_be_bytes())?;

        let peer_port = self.state.client.encrypted_reader.read(self.state.timeout)?;
        let peer_port = peer_port.try_into().unwrap();
        let peer_port = u16::from_be_bytes(peer_port);

        Ok(peer_port)
    }

    fn collect_samples(&mut self, amount: u8) -> Result<(), Box<dyn Error>> {
        for i in 0..amount {
            let start = SystemTime::now();

            self.state.client.encrypted_writer.write(&[amount - i - 1])?;

            let time = self.state.client.encrypted_reader.read(self.state.timeout)?;

            let now = SystemTime::now();
            let now_nanos = now.duration_since(UNIX_EPOCH)?.as_nanos();
            let time = time.try_into().unwrap();
            let peer_now_nanos = u128::from_be_bytes(time);
            let elapsed_nanos = start.elapsed()?.as_nanos();
            let diff = now_nanos as i128 - peer_now_nanos as i128 - elapsed_nanos as i128 / 2; // Zeitdifferenz symmetrischer Jitter

            self.state.client.clock_diff_samples.push(diff);

            if self.state.client.max_delay < elapsed_nanos {
                self.state.client.max_delay = elapsed_nanos;
            }
        }

        Ok(())
    }

    fn provide_samples(&mut self) -> Result<(), Box<dyn Error>> {
        loop {
            let num = self.state.client.encrypted_reader.read(self.state.timeout)?;

            let now = SystemTime::now();
            let now_nanos = now.duration_since(UNIX_EPOCH)?.as_nanos();

            self.state.client.encrypted_writer.write(now_nanos.to_be_bytes().as_slice())?;

            if num[0] == 0 {
                return Ok(());
            }
        }
    }
}

impl<P: ProtocolState> Connection<Active<Plain<P>>> {
    pub fn accept(self) -> (P::Writer, P::Reader) {
        (self.state.client.plain_writer, self.state.client.plain_reader)
    }
}
impl<P: ProtocolState> Connection<Active<Encrypted<P>>> {
    pub fn accept(self) -> (EncryptedWriter<P::Writer>, EncryptedReader<P::Reader>) {
        (self.state.client.encrypted_writer, self.state.client.encrypted_reader)
    }
}

impl Connection<Active<Encrypted<Udp>>> {}

pub struct ChangeStateError<C>(pub(crate) C, Box<dyn Error>);

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

impl<C> Error for ChangeStateError<C> {}

impl<C> ChangeStateError<C> {
    pub fn new(state: C, err: Box<dyn Error>) -> ChangeStateError<C> {
        ChangeStateError(state, err)
    }

    pub fn to_state(self) -> C {
        self.0
    }

    pub fn to_err(self) -> Box<dyn Error> {
        self.1
    }

    pub fn state_ref(&mut self) -> &mut C {
        &mut self.0
    }

    pub fn err_ref(&mut self) -> &mut Box<dyn Error> {
        &mut self.1
    }

    pub fn split(self) -> (C, Box<dyn Error>) {
        (self.0, self.1)
    }
}

#[cfg(test)]
mod tests {
    use std::thread;

    use dryoc::dryocstream::Tag;

    use super::*;

    #[test]
    fn test_connect_err() {
        let timeout = Duration::from_millis(10);

        let c1 = Connection::<Waiting>::new(None).unwrap();
        let c2 = Connection::<Waiting>::new(None).unwrap();

        let ipv6 = Ipv6Addr::from(1);

        assert!(c1.connect(ipv6, c2.get_port(), Some(timeout), Some(timeout)).is_err());
    }

    #[test]
    fn test_connect_ok() {
        let timeout = Duration::from_millis(100);

        let c1 = Connection::<Waiting>::new(None).unwrap();
        let c2 = Connection::<Waiting>::new(None).unwrap();

        let p1 = c1.get_port();
        let p2 = c2.get_port();

        let ipv6 = Ipv6Addr::from(1);

        let thread_c2 = thread::spawn(move || {
            return c2.connect(ipv6, p1, Some(timeout), Some(timeout)).is_ok();
        });

        assert!(c1.connect(ipv6, p2, Some(timeout), Some(timeout)).is_ok());
        assert!(thread_c2.join().unwrap());
    }

    fn connect() -> (Connection<Active<Plain<Udp>>>, Connection<Active<Plain<Udp>>>) {
        let timeout = Duration::from_millis(100);

        let c1 = Connection::<Waiting>::new(None).unwrap();
        let c2 = Connection::<Waiting>::new(None).unwrap();

        let p1 = c1.get_port();
        let p2 = c2.get_port();

        let ipv6 = Ipv6Addr::from(1);

        let thread_c2 = thread::spawn(move || {
            return c2.connect(ipv6, p1, Some(timeout), Some(timeout)).unwrap();
        });

        let c1 = c1.connect(ipv6, p2, Some(timeout), Some(timeout)).unwrap();
        let c2 = thread_c2.join().unwrap();

        (c1, c2)
    }

    #[test]
    fn test_negotiate_roles() {
        let (mut c1, mut c2) = connect();

        //260 because test with message counter overflow (max msg count = 255)
        for _ in 0..260 {
            let thread_c2 = thread::spawn(move || {
                c2.negotiate_roles().unwrap();
                return c2;
            });

            c1.negotiate_roles().unwrap();
            c2 = thread_c2.join().unwrap();

            assert_ne!(c1.state.role, c2.state.role);
            assert_ne!(c1.state.role, Role::None);
            assert_ne!(c2.state.role, Role::None);
        }
    }

    #[test]
    fn test_exchange_keys() {
        let (mut c1, mut c2) = connect();

        let thread_c2 = thread::spawn(move || {
            c2.negotiate_roles().unwrap();
            return c2;
        });

        c1.negotiate_roles().unwrap();
        c2 = thread_c2.join().unwrap();


        let thread_c2 = thread::spawn(move || {
            return c2.exchange_keys().unwrap();
        });

        let (c1_decrypt_key, c1_encrypt_key) = c1.exchange_keys().unwrap();
        let (c2_decrypt_key, c2_encrypt_key) = thread_c2.join().unwrap();

        assert_eq!(c1_decrypt_key, c2_encrypt_key);
        assert_eq!(c2_decrypt_key, c1_encrypt_key);
        assert_ne!(c1_decrypt_key, c1_encrypt_key);
    }

    #[test]
    fn test_generate_crypto_streams() {
        let (mut c1, mut c2) = connect();

        let thread_c2 = thread::spawn(move || {
            c2.negotiate_roles().unwrap();
            let (c2_decrypt_key, c2_encrypt_key) = c2.exchange_keys().unwrap();

            return c2.generate_crypto_streams(c2_decrypt_key, c2_encrypt_key).unwrap();
        });

        c1.negotiate_roles().unwrap();
        let (c1_decrypt_key, c1_encrypt_key) = c1.exchange_keys().unwrap();

        let (mut c1_pull, mut c1_push) = c1.generate_crypto_streams(c1_decrypt_key, c1_encrypt_key).unwrap();
        let (mut c2_pull, mut c2_push) = thread_c2.join().unwrap();

        let c1_msg = b"Hallo wie gehts c2?";
        let c2_msg = b"Hallo wie gehts c1?";

        let c1_enc = c1_push.push_to_vec(c1_msg, None, Tag::MESSAGE).unwrap();
        let c2_enc = c2_push.push_to_vec(c2_msg, None, Tag::MESSAGE).unwrap();

        let (c1_dec, _) = c2_pull.pull_to_vec(&c1_enc, None).unwrap();
        let (c2_dec, _) = c1_pull.pull_to_vec(&c2_enc, None).unwrap();

        assert_eq!(c1_dec, c1_msg);
        assert_eq!(c2_dec, c2_msg);
    }

    #[test]
    fn test_encrypt_udp() {
        let (c1, c2) = connect();

        let thread_c2 = thread::spawn(move || {
            return c2.encrypt().unwrap();
        });

        let _c1 = c1.encrypt().unwrap();
        let _c2 = thread_c2.join().unwrap();
    }

    #[test]
    fn test_exchange_ports() {
        let (c1, c2) = connect();

        let thread_c2 = thread::spawn(move || {
            let mut c2 = c2.encrypt().unwrap();
            return c2.exchange_ports(1000).unwrap();
        });

        let mut c1 = c1.encrypt().unwrap();
        let p1 = c1.exchange_ports(2000).unwrap();
        let p2 = thread_c2.join().unwrap();

        assert_eq!(p1, 1000);
        assert_eq!(p2, 2000);
    }

    #[test]
    fn test_exchange_samples() {
        let (c1, c2) = connect();

        let thread_c2 = thread::spawn(move || {
            let mut c2 = c2.encrypt().unwrap();
            c2.collect_samples(255).unwrap();
            return c2;
        });

        let mut c1 = c1.encrypt().unwrap();
        c1.provide_samples().unwrap();
        let c2 = thread_c2.join().unwrap();

        assert_eq!(c2.state.client.clock_diff_samples.len(), 255);
        assert_eq!(c1.state.client.clock_diff_samples.len(), 0);
    }

    #[test]
    fn test_exchange_connect_time() {
        let (c1, c2) = connect();

        let thread_c2 = thread::spawn(move || {
            let mut c2 = c2.encrypt().unwrap();
            c2.collect_samples(255).unwrap();
            return c2.set_connect_time().unwrap();
        });

        let mut c1 = c1.encrypt().unwrap();
        c1.provide_samples().unwrap();
        let t1 = c1.get_connect_time().unwrap();
        let t2 = thread_c2.join().unwrap();

        println!("{}", ((t1.as_nanos() as i128 - t2.as_nanos() as i128).abs() as u128));
        assert!(((t1.as_nanos() as i128 - t2.as_nanos() as i128).abs() as u128) < Duration::from_millis(1).as_nanos());
    }

    fn try_upgrade_tcp(mut c1: Connection<Active<Encrypted<Udp>>>, mut c2: Connection<Active<Encrypted<Udp>>>, tries: u8) -> Result<(Connection<Active<Encrypted<Tcp>>>, Connection<Active<Encrypted<Tcp>>>), (Connection<Active<Encrypted<Udp>>>, Connection<Active<Encrypted<Udp>>>)>{
        let thread_c2 = thread::spawn(move || {
            for _ in 0..tries {
                c2 = match c2.upgrade() {
                    Ok(c) => return Ok(c),
                    Err(err) => err.0,
                };
            }

            Err(c2)
        });

        let thread_c1 = thread::spawn(move || {
            for _ in 0..tries {
                c1 = match c1.upgrade() {
                    Ok(c) => return Ok(c),
                    Err(err) => err.0,
                };
            }

            Err(c1)
        });

        let c1 = thread_c1.join().unwrap();
        let c2 = thread_c2.join().unwrap();


        match (c1, c2) {
            (Ok(c1), Ok(c2)) => {
                return Ok((c1, c2));
            },
            (Err(c1), Err(c2)) => {
                return Err((c1, c2));
            }
            (_, _) => panic!("c1 and c2 do not match in Tcp/Udp type"),
        }

    }

    #[test]
    fn test_upgrade_tcp() {
        let (c1, c2) = connect();

        let thread_c2 = thread::spawn(move || {
            let mut c2 = c2.encrypt().unwrap();
            return c2;
        });
        let mut c1 = c1.encrypt().unwrap();
        let mut c2 = thread_c2.join().unwrap();

        assert!(try_upgrade_tcp(c1, c2, 10).is_ok());
    }

    #[test]
    fn test_read_writer_encrypted_tcp() {
        let (c1, c2) = connect();

        let thread_c2 = thread::spawn(move || {
            let c2 = c2.encrypt().unwrap();
            return c2;
        });
        let c1 = c1.encrypt().unwrap();
        let c2 = thread_c2.join().unwrap();

        let(c1, c2) = match try_upgrade_tcp(c1, c2, 10) {
            Ok(cc) => cc,
            Err(_) => panic!("failed to upgrade")
        };

        let (mut c1_writer, mut c1_reader) = c1.accept();
        let (mut c2_writer, mut c2_reader) = c2.accept();

        let c1_msg = b"Das ist ein Test. Diese Nachricht wird von c1 an c2 versendet.";
        let c2_msg = b"Das ist ein Test. Diese Nachricht wird von c2 an c1 versendet.";

        c1_writer.write(c1_msg).unwrap();
        c2_writer.write(c2_msg).unwrap();

        let c1_recv = c1_reader.try_read().unwrap();
        let c2_recv = c2_reader.try_read().unwrap();

        assert_eq!(c1_msg.as_slice(), c2_recv.as_slice());
        assert_eq!(c1_msg.as_slice(), c2_recv.as_slice());
    }
}