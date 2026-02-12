use log::{LevelFilter, info};
use std::time::Duration;

#[path = "../examples/lib/common.rs"]
mod common;
use common::logger_builder;

#[allow(unreachable_code)]
fn main() -> std::io::Result<()> {
    logger_builder(LevelFilter::Debug);

    let serial = serialport::new("/dev/ttyUSB0", 115200)
        // .parity(serialport::Parity::Even)
        .timeout(Duration::from_millis(300))
        .open()?;

    // Indirizzo IP e porta del lettore UHF
    // let addr = "192.168.0.178:4001";
    //
    // info!("Tentativo di connessione a {}...", addr);
    //
    // // Apertura della connessione TCP con un timeout
    // let stream = TcpStream::connect_timeout(&addr.parse().unwrap(), Duration::from_secs(5))?;
    // stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    // stream.set_write_timeout(Some(Duration::from_secs(2)))?;

    info!("Connesso con successo!");

    common::test_connection(serial)
}
