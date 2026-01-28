use e710_uhf::connector::Connector;
use e710_uhf::frame::command::Command;
use e710_uhf::frequency_references::Spectrum;
use log::{Level, LevelFilter, debug, error, info, warn};
use std::io::Write;
use std::net::TcpStream;
use std::thread::sleep;
use chrono::{DateTime, Utc};
use std::time::{Duration, UNIX_EPOCH};

fn ns_to_iso(ts_ns: u64) -> String {
    let secs = ts_ns / 1_000_000_000;
    let nanos = (ts_ns % 1_000_000_000) as u32;

    let dt = DateTime::<Utc>::from(UNIX_EPOCH + Duration::new(secs, nanos));
    dt.to_rfc3339()
}

fn logger_builder(level: LevelFilter) {
    let mut builder = env_logger::Builder::new();
    builder
        .filter_level(level)
        .format(|buf, record| {
            let tm = buf.timestamp();
            let level_string = match record.level() {
                Level::Warn => "⚠️ WARNING",
                Level::Info => "ℹ️ INFO",
                l => l.as_str(),
            };
            writeln!(buf, "T{tm} [{level_string}]: {}", record.args())
        })
        .write_style(env_logger::fmt::WriteStyle::Always)
        .init();
}

fn main() -> std::io::Result<()> {
    logger_builder(LevelFilter::Info);

    // Indirizzo IP e porta del lettore UHF
    let addr = "192.168.0.178:4001";

    info!("Tentativo di connessione a {}...", addr);

    // Apertura della connessione TCP con un timeout
    let stream = TcpStream::connect_timeout(&addr.parse().unwrap(), Duration::from_secs(5))?;

    info!("Connesso con successo!");

    // Creazione del connector utilizzando lo stream TCP
    let mut connector = Connector::new(stream, 8, vec![25], (Spectrum::ETSI, 865.0, 868.0));

    connector.setup_reader().unwrap();

    // Vettore di comandi da eseguire
    let commands = vec![
        Command::GetFirmwareVersion,
        Command::SetWorkAntenna(0),
        Command::GetWorkAntenna,
        Command::GetReaderTemperature,
        Command::GetRfPortReturnLoss(866.0),
    ];

    // Ciclo sui comandi
    for cmd in commands {
        let response = connector.send_and_read_command(cmd).unwrap();

        // Lettura della risposta
        info!("Risposta ricevuta: {response}\n");

        sleep(Duration::from_secs(1));
    }

    info!("Get Antenna statistics");
    let statistics = connector.get_statistic_to_all_antennas().unwrap();

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

    info!("\n\n== Avvio lettore fast switching:");
    loop {
        debug!("Waiting for tags...");

        let mut iter_tag = connector.new_fast_switching_antenna_iterator().unwrap();
        while let Some(res) = iter_tag.next() {
            match res {
                Ok(tag) => {
                    info!("- { } {tag}", ns_to_iso(tag.received_at_ns));
                }
                Err(e) => error!("Error reading tags: {:?}", e),
            }
        }
        sleep(Duration::from_millis(30));
    }

    sleep(Duration::from_secs(4));

    Ok(())
}
