use dryoc::dryocstream::{DryocStream, Pull, Push};
use crate::client::udp::UdpActiveClient;

pub struct EncryptedClient {
    client: UdpActiveClient,
    decrypt_stream: DryocStream<Pull>,
    encrypt_stream: DryocStream<Push>,
}

impl EncryptedClient {




}