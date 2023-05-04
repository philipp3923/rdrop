use dryoc::dryocstream::{DryocStream, Pull, Push};
use crate::udp::ActiveClient;

pub struct EncryptedClient {
    client: ActiveClient,
    decrypt_stream: DryocStream<Pull>,
    encrypt_stream: DryocStream<Push>,
}

impl EncryptedClient {




}