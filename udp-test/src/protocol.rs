use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::net::Ipv6Addr;
use std::time::Duration;

use dryoc::dryocbox::{Bytes, KeyPair};
use dryoc::dryocstream::{DryocStream, Header, Pull, Push};
use dryoc::kx::{Session, SessionKey};
use dryoc::sign::PublicKey;
use rand::{Rng, thread_rng};

use crate::client::{ActiveClient, ClientReader, ClientWriter, EncryptedClient, EncryptedReader, EncryptedWriter};
use crate::client::tcp::{TcpClientReader, TcpClientWriter};
use crate::client::udp::{UdpActiveClient, UdpClientReader, UdpClientWriter, UdpWaitingClient};

pub trait EncryptionState {}

pub trait ConnectionState {}

pub trait ProtocolState {
    type Writer: ClientWriter;
    type Reader: ClientReader;
}

pub struct Encrypted<P: ProtocolState> {
    encrypted_reader: EncryptedReader<P::Reader>,
    encrypted_writer: EncryptedWriter<P::Writer>,
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
    encryption: E,
}

pub struct Waiting {
    waiting_client: UdpWaitingClient,
}

impl<E: EncryptionState> ConnectionState for Active<E> {}

impl ConnectionState for Waiting {}

pub struct Udp {}

pub struct Tcp {}

impl ProtocolState for Udp {
    type Reader = UdpClientReader;
    type Writer = UdpClientWriter;
}

impl ProtocolState for Tcp {
    type Reader = TcpClientReader;
    type Writer = TcpClientWriter;
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

        Ok(Connection::<Active<Plain<Udp>>>::new(udp_active_client, disconnect_timeout))
    }
}

impl Connection<Active<Plain<Udp>>> {
    fn new(udp_active_client: UdpActiveClient, timeout: Option<Duration>) -> Connection<Active<Plain<Udp>>> {
        let (writer, reader) = udp_active_client.split();

        Connection { state: Active { timeout, role: Role::None, encryption: Plain { plain_reader: reader, plain_writer: writer } } }
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

        let encrypted_reader = EncryptedReader::new(pull_stream, self.state.encryption.plain_reader);
        let encrypted_writer = EncryptedWriter::new(push_stream, self.state.encryption.plain_writer);

        let connection = Connection::<Active<Encrypted<Udp>>> { state: Active { role: self.state.role, timeout: self.state.timeout, encryption: Encrypted { encrypted_writer, encrypted_reader } } };

        Ok(connection)
    }

    fn exchange_keys(&mut self) -> Result<(SessionKey, SessionKey), Box<dyn Error>> {
        assert_ne!(self.state.role, Role::None);

        let my_keypair = KeyPair::gen();

        self.state.encryption.plain_writer.write(my_keypair.public_key.as_slice())?;

        let peer_public_key = self.state.encryption.plain_reader.read(self.state.timeout)?;

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

        self.state.encryption.plain_writer.write(header.as_slice())?;

        let header = self.state.encryption.plain_reader.read(self.state.timeout)?;

        let pull_stream = DryocStream::init_pull(&decrypt_key, &header);

        return Ok((pull_stream, push_stream));
    }

    fn negotiate_roles(&mut self) -> Result<(), Box<dyn Error>> {
        let mut rng = thread_rng();

        loop {
            let my_number: [u8; 2] = [rng.gen(), rng.gen()];
            self.state.encryption.plain_writer.write(my_number.as_slice())?;
            let peer_number = self.state.encryption.plain_reader.read(self.state.timeout)?;

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
    pub fn upgrade(self) -> Result<Connection<Active<Encrypted<Tcp>>>, ChangeStateError<Self>> {
        todo!()
    }
}

impl<P: ProtocolState> Connection<Active<Plain<P>>> {
    pub fn accept(self) -> (P::Writer, P::Reader) {
        (self.state.encryption.plain_writer, self.state.encryption.plain_reader)
    }
}

impl<P: ProtocolState> Connection<Active<Encrypted<P>>> {
    pub fn accept(self) -> (EncryptedWriter<P::Writer>, EncryptedReader<P::Reader>) {
        (self.state.encryption.encrypted_writer, self.state.encryption.encrypted_reader)
    }
}


pub struct ChangeStateError<C>(C, Box<dyn Error>);

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
    fn test_negotiate_exchange_keys() {
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
    fn test_encrypt_udp(){
        let (mut c1, mut c2) = connect();

        let thread_c2 = thread::spawn(move || {
            return c2.encrypt().unwrap();
        });

        let mut c1 = c1.encrypt().unwrap();
        let mut c2 = thread_c2.join().unwrap();
    }
}