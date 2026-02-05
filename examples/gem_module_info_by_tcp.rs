use std::net::TcpStream;
use std::time::Duration;
use log::{info, LevelFilter};
use crate::common::logger_builder;

#[path = "../examples/lib/common.rs"]
mod common;

fn main() -> std::io::Result<()> {
    logger_builder(LevelFilter::Debug);
    
    // Indirizzo IP e porta del lettore UHF
    let addr = "192.168.0.178:4001";

    info!("Tentativo di connessione a {}...", addr);

    // Apertura della connessione TCP con un timeout
    let stream = TcpStream::connect_timeout(&addr.parse().unwrap(), Duration::from_secs(5))?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;

    info!("Connesso con successo!");
    common::test_connection(stream)
}