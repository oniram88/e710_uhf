//! Finite hardware-in-the-loop checks for an E710 reader.
//!
//! Examples:
//!   cargo run --example hil -- rs232 /dev/ttyUSB0
//!   cargo run --example hil -- tcp 192.168.0.178:4001 --antenna
//!   cargo run --example hil -- rs232 /dev/ttyUSB0 --inventory --require-tag

#[path = "lib/hil.rs"]
mod hil;
#[path = "lib/hil_args.rs"]
mod hil_args;
#[cfg(feature = "async")]
#[path = "lib/hil_continuous.rs"]
mod hil_continuous;

use hil_args::{Endpoint, parse_args, usage};
use serialport::{DataBits, FlowControl, Parity, StopBits};
use std::env;
use std::net::{SocketAddr, TcpStream};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if hil_args::is_help(&args) {
        println!("{}", usage("hil"));
        return ExitCode::SUCCESS;
    }

    match parse_args(args, false) {
        Ok(config) => match execute(config) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("HIL error: {error}");
                ExitCode::FAILURE
            }
        },
        Err(error) => {
            eprintln!("{error}\n\n{}", usage("hil"));
            ExitCode::FAILURE
        }
    }
}

fn execute(config: hil_args::Config) -> Result<(), String> {
    let report = match config.endpoint {
        Endpoint::Rs232(device) => {
            println!(
                "Running E710 HIL via RS-232 on {device} (timeout {} ms)",
                config.options.timeout.as_millis()
            );
            let serial = serialport::new(&device, 115_200)
                .data_bits(DataBits::Eight)
                .parity(Parity::None)
                .stop_bits(StopBits::One)
                .flow_control(FlowControl::None)
                .timeout(config.options.timeout)
                .open()
                .map_err(|error| format!("cannot open serial device {device}: {error}"))?;
            hil::run(serial, &config.options)
        }
        Endpoint::Tcp(address) => {
            println!(
                "Running E710 HIL via TCP on {address} (timeout {} ms)",
                config.options.timeout.as_millis()
            );
            let socket_address: SocketAddr = address
                .parse()
                .map_err(|error| format!("invalid TCP address {address}: {error}"))?;
            let stream = TcpStream::connect_timeout(&socket_address, config.options.timeout)
                .map_err(|error| format!("cannot connect to {address}: {error}"))?;
            stream
                .set_read_timeout(Some(config.options.timeout))
                .map_err(|error| format!("cannot configure TCP read timeout: {error}"))?;
            stream
                .set_write_timeout(Some(config.options.timeout))
                .map_err(|error| format!("cannot configure TCP write timeout: {error}"))?;
            hil::run(stream, &config.options)
        }
    };

    report.print_summary();
    if report.is_success() {
        Ok(())
    } else {
        Err("one or more hardware checks failed".to_owned())
    }
}
