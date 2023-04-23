#[repr(u8)]
pub enum Opcode {
    RequestSend = 0x01, // Request to send a file
    AcceptFile =  0x02, // Accept a file
    DenyFile = 0x03, // Deny a file
    RequestFileChunk = 0x04, // Request a chunk of a file
    Message = 0x05 // Text message
}

impl TryFrom<u8> for Opcode {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            x if x == Opcode::RequestSend as u8 => Ok(Opcode::RequestSend),
            x if x == Opcode::AcceptFile as u8 => Ok(Opcode::AcceptFile),
            x if x == Opcode::DenyFile as u8 => Ok(Opcode::DenyFile),
            x if x == Opcode::RequestFileChunk as u8 => Ok(Opcode::RequestFileChunk),
            x if x == Opcode::Message as u8 => Ok(Opcode::Message),
            _ => Err(()),
        }
    }
}