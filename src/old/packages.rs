use std::mem::size_of;
use std::net::TcpStream;
use crate::opcode::Opcode;

pub enum Package {
    RequestSend{file_id: u32, file_size: u128, file_name: [u8; 100], hash: u32}
}


/*impl TryFrom<(Opcode, &mut TcpStream)> for Package {
    type Error = ();

    fn try_from(value: (Opcode, &mut TcpStream)) -> Result<Self, Self::Error> {
        match value.0 {
            Opcode::RequestSend => {
                let mut buffer: [u8;]
            }
            Opcode::AcceptFile => {

            }
            Opcode::DenyFile => {

            }
            Opcode::RequestFileChunk => {

            }
            Opcode::Message => {

            }
        }
    }
}*/