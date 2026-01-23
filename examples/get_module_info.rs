use std::net::TcpStream;
use std::thread::sleep;
use std::time::Duration;
use e710_uhf::connector::Connector;
use e710_uhf::frame::{Command, Frame};

fn main() -> std::io::Result<()> {
    // Indirizzo IP e porta del lettore UHF
    let addr = "192.168.0.178:4001";
    
    println!("Tentativo di connessione a {}...", addr);
    
    // Apertura della connessione TCP con un timeout
    let stream = TcpStream::connect_timeout(
        &addr.parse().unwrap(),
        Duration::from_secs(5)
    )?;
    
    println!("Connesso con successo!");

    // Creazione del connector utilizzando lo stream TCP
    let mut connector = Connector::new(stream);

    loop {
        // Vettore di comandi da eseguire
        let commands = vec![
            Command::GetFirmwareVersion,
            Command::GetTemperature,
            Command::GetFrequencyRegion
        ];

        // Ciclo sui comandi
        for cmd in commands {
            connector.send_command(cmd.clone()).unwrap();

            // Lettura della risposta
            let response = connector.read_command().unwrap();
            println!("Risposta ricevuta: {response}");

            sleep(Duration::from_secs(1));
        }

        sleep(Duration::from_secs(4));

    }
    Ok(())
}
