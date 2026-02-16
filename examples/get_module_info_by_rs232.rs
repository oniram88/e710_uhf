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


    info!("Connesso con successo!");

    common::test_connection(serial)
}
