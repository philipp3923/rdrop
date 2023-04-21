extern crate core;

use std::error::Error;

mod connection;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut connection = connection::Connection::new("2001:7c7:2159:2f00:79da:1d32:cee6:f77",2000).unwrap();

    connection.connect(10).await;

    return Ok(());
}
