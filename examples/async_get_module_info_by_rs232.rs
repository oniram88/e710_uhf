use futures_util::stream::StreamExt; // for `.next().await` on streams
use log::{LevelFilter, debug, error, info};
use serialport::{DataBits, FlowControl, Parity, StopBits};
use std::time::Duration;
use tokio::time::sleep;
use tokio_serial::SerialPortBuilderExt;

#[path = "../examples/lib/common.rs"]
mod common;
use common::logger_builder;

use crate::common::ns_to_iso;
use e710_uhf::connector::AsyncIO;
use e710_uhf::connector::Connector;
use e710_uhf::frame::command::{BeeperMode, Command};
use e710_uhf::frequency_references::Spectrum;

#[allow(unreachable_code)]
#[tokio::main]
async fn main() -> std::io::Result<()> {
    logger_builder(LevelFilter::Info);

    let serial = tokio_serial::new("/dev/ttyUSB0", 115200)
        .data_bits(DataBits::Eight)
        .parity(Parity::None)
        .stop_bits(StopBits::One)
        .flow_control(FlowControl::None)
        .open_native_async()?;

    // let serial = serialport::open(&tokio_serial::new("/dev/ttyUSB0", 115200))
    //     .expect("Failed to open async serial port");

    info!("Connesso con successo!");

    // Creazione del connector utilizzando lo stream TCP
    let mut connector = Connector::new(
        serial,
        8,
        vec![25],
        (Spectrum::ETSI, 865.0, 868.0),
        Some(Duration::from_millis(200)),
    );

    connector.setup_reader().await.unwrap();

    // Vettore di comandi da eseguire
    let commands = vec![
        Command::GetFirmwareVersion,
        Command::SetWorkAntenna(0),
        Command::GetWorkAntenna,
        Command::GetReaderTemperature,
        Command::GetRfPortReturnLoss(866.0),
        Command::SetBeeperMode(BeeperMode::Quiet),
    ];

    // Ciclo sui comandi
    for cmd in commands {
        let response = connector.send_and_read_command(cmd).await.unwrap();

        // Lettura della risposta
        info!("Risposta ricevuta: {response}\n");

        sleep(Duration::from_millis(5)).await;
    }

    info!("Get Antenna statistics");
    let statistics = connector.get_statistic_to_all_antennas().await.unwrap();

    info!("| ID antenna | vswr |");
    for (id_antenna, vswr) in statistics.iter() {
        info!("| {id_antenna} | {vswr} |");
    }

    // info!("\n\n== Avvio lettore:");
    // loop {
    //  let results =  connector.make_a_read_single_antenna().unwrap();
    //
    //     if results.len() > 0 {
    //         info!("{:?}", results);
    //     }
    //     sleep(Duration::from_millis(30));
    // }

    info!("Build automatic antennas_cfgs");
    let cfgs = connector.build_fast_switching_antenna_cfg(1).await.unwrap();
    info!("Configuration: {:#?}", cfgs);

    info!("\n\n== Avvio lettore fast switching:");
    loop {
        debug!("Waiting for tags...");

        // Streams produced by async_stream are not Unpin by default; pin the stream
        let mut iter_tag = Box::pin(connector.new_fast_switching_antenna_iterator(cfgs.clone()));
        while let Some(res) = iter_tag.as_mut().next().await {
            match res {
                Ok(tag) => {
                    info!("- { } {tag}", ns_to_iso(tag.received_at_ns));
                }
                Err(e) => error!("Error reading tags: {:?}", e),
            }
        }
        sleep(Duration::from_millis(30)).await;
    }

    // sleep(Duration::from_secs(4));

    Ok(())
}
