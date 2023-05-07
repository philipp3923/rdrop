use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::net::Ipv6Addr;
use std::time::Duration;
use crate::client::{ActiveClient, ClientReader, ClientWriter, EncryptedReader, EncryptedWriter};
use crate::client::tcp::{TcpClientReader, TcpClientWriter};
use crate::client::udp::{UdpActiveClient, UdpClientReader, UdpClientWriter, UdpWaitingClient};

pub trait EncryptionState {}
pub trait ConnectionState {
}
pub trait ProtocolState {
    type Writer: ClientWriter;
    type Reader: ClientReader;
}

pub struct Encrypted<P: ProtocolState> {
    encrypted_reader: EncryptedReader<P::Reader>,
    encrypted_writer: EncryptedWriter<P::Writer>
}
pub struct Plain<P: ProtocolState> {
    plain_reader: P::Reader,
    plain_writer: P::Writer
}
impl<P: ProtocolState> EncryptionState for Encrypted<P> {}
impl<P: ProtocolState> EncryptionState for Plain<P> {}

pub struct Active<E: EncryptionState> {
    encryption: E
}
pub struct Waiting {
    waiting_client: UdpWaitingClient
}
impl<E: EncryptionState> ConnectionState for Active<E> {}
impl ConnectionState for Waiting{}

pub struct Udp{}
pub struct Tcp{}

impl ProtocolState for Udp {
    type Reader = UdpClientReader;
    type Writer = UdpClientWriter;
}
impl ProtocolState for Tcp {
    type Reader = TcpClientReader;
    type Writer = TcpClientWriter;
}

pub struct Connection<C: ConnectionState> {
    state: C
}

impl Connection<Waiting> {

    pub fn new(port: Option<u16>) -> Result<Connection<Waiting>, Box<dyn Error>>{
        let waiting_client = UdpWaitingClient::new(port)?;
        let state = Waiting { waiting_client };
        Ok(Connection {state})
    }

    pub fn get_port(&self) -> u16 {
        self.state.waiting_client.get_port()
    }

    pub fn connect(
        self,
        peer: Ipv6Addr,
        port: u16,
        connect_timeout: Option<Duration>,
        disconnect_timeout: Option<Duration>
    ) -> Result<Connection<Active<Plain<Udp>>>, ChangeStateError<Self>> {
        return match self.state.waiting_client.connect(peer,port,connect_timeout, disconnect_timeout) {
            Ok(connection) => Ok(Connection::<Active<Plain<Udp>>>::new(connection)),
            Err(err) => {
                let err = err.split();
                return Err(ChangeStateError(Connection {state: Waiting{waiting_client: err.0}}, err.1)) },
        }

    }
}

impl Connection<Active<Plain<Udp>>> {
    fn new(udp_active_client: UdpActiveClient) -> Connection<Active<Plain<Udp>>> {
        let (writer, reader) = udp_active_client.split();

        Connection {state : Active{encryption: Plain {plain_reader: reader, plain_writer: writer}}}
    }

    pub fn encrypt(self) -> Result<Connection<Active<Encrypted<Udp>>>, ChangeStateError<Self>> {
        todo!()
    }

    pub fn accept(self) -> (UdpClientWriter, UdpClientReader) {
        (self.state.encryption.plain_writer, self.state.encryption.plain_reader)
    }
}

impl Connection<Active<Encrypted<Udp>>> {

    pub fn upgrade(self) -> Result<Connection<Active<Encrypted<Tcp>>>, ChangeStateError<Self>> {
        todo!()
    }

    pub fn accept(self) -> (EncryptedWriter<UdpClientWriter>, EncryptedReader<UdpClientReader>) {
        todo!()
    }
}

impl Connection<Active<Encrypted<Tcp>>> {


    pub fn accept(self) -> (EncryptedWriter<TcpClientWriter>, EncryptedReader<TcpClientReader>) {
        todo!()
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

impl<C> Error for ChangeStateError<C> {

}

impl<C> ChangeStateError<C> {

    pub fn new(state: C, err: Box<dyn Error>) -> ChangeStateError<C>{
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


}