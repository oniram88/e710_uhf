use e710_uhf::connector::Connector;
use e710_uhf::frame::Command;
use e710_uhf::frequency_references::{get_param, Spectrum};
use std::net::TcpStream;
use std::thread::sleep;
use std::time::Duration;

fn main() -> std::io::Result<()> {
    // Indirizzo IP e porta del lettore UHF
    let addr = "192.168.0.178:4001";

    println!("Tentativo di connessione a {}...", addr);

    // Apertura della connessione TCP con un timeout
    let stream = TcpStream::connect_timeout(&addr.parse().unwrap(), Duration::from_secs(5))?;

    println!("Connesso con successo!");

    // Creazione del connector utilizzando lo stream TCP
    let mut connector = Connector::new(stream,8);

    // Vettore di comandi da eseguire
    let commands = vec![
        Command::GetFirmwareVersion,
        Command::GetWorkAntenna,
        Command::GetReaderTemperature,
        Command::GetRfPortReturnLoss(866.0)
    ];

    // Ciclo sui comandi
    for cmd in commands {
        connector.send_command(cmd.clone()).unwrap();

        // Lettura della risposta
        let response = connector.read_command().unwrap();
        println!("Risposta ricevuta: {response}\n");

        sleep(Duration::from_secs(1));
    }

    println!("\n\n== Controllo antenna detection:");
    connector.set_ant_connection_detector_if_not(0x03).unwrap();

    println!("\n\n== Controllo frequenza:");
    connector
        .set_frequency_if_not(Spectrum::ETSI, 865.0, 868.0)
        .unwrap();

    println!("\n\n== Controllo potenza:");
    connector.set_output_power_if_not(vec![15]).unwrap();

    println!("\n\n== Controllo tutte le antenne:");
    connector.check_all_antennas_rf_port_return_loss(866.0).unwrap();

    sleep(Duration::from_secs(4));

    Ok(())
}
