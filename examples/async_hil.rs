//! Async finite hardware-in-the-loop checks for an E710 reader.
//!
//! Examples:
//!   cargo run --example async_hil --features async -- rs232 /dev/ttyUSB0
//!   cargo run --example async_hil --features async -- tcp 192.168.0.178:4001 --antenna
//!   cargo run --example async_hil --features async -- rs232 /dev/ttyUSB0
//!     --antenna --inventory --duration-secs 120 --output inventory.csv

#[cfg(not(feature = "async"))]
compile_error!("the async_hil example requires `--features async`");

#[path = "lib/hil.rs"]
mod hil;
#[path = "lib/hil_args.rs"]
mod hil_args;
#[path = "lib/hil_continuous.rs"]
mod hil_continuous;

use hil_args::{Endpoint, parse_args, usage};
use std::env;
use std::process::ExitCode;
use tokio::net::TcpStream;
use tokio_serial::{DataBits, FlowControl, Parity, SerialPortBuilderExt, StopBits};

#[tokio::main]
async fn main() -> ExitCode {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if hil_args::is_help(&args) {
        println!("{}", usage("async_hil"));
        return ExitCode::SUCCESS;
    }

    match parse_args(args, true) {
        Ok(config) => match execute(config).await {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("Async HIL error: {error}");
                ExitCode::FAILURE
            }
        },
        Err(error) => {
            eprintln!("{error}\n\n{}", usage("async_hil"));
            ExitCode::FAILURE
        }
    }
}

async fn execute(config: hil_args::Config) -> Result<(), String> {
    let report = match config.endpoint {
        Endpoint::Rs232(device) => {
            println!(
                "Running async E710 HIL via RS-232 on {device} (timeout {} ms)",
                config.options.timeout.as_millis()
            );
            let serial = tokio_serial::new(&device, 115_200)
                .data_bits(DataBits::Eight)
                .parity(Parity::None)
                .stop_bits(StopBits::One)
                .flow_control(FlowControl::None)
                .open_native_async()
                .map_err(|error| format!("cannot open serial device {device}: {error}"))?;
            hil::run_async(serial, &config.options).await
        }
        Endpoint::Tcp(address) => {
            println!(
                "Running async E710 HIL via TCP on {address} (timeout {} ms)",
                config.options.timeout.as_millis()
            );
            let stream = tokio::time::timeout(config.options.timeout, TcpStream::connect(&address))
                .await
                .map_err(|_| format!("connection to {address} timed out"))?
                .map_err(|error| format!("cannot connect to {address}: {error}"))?;
            hil::run_async(stream, &config.options).await
        }
    };

    report.print_summary();
    if report.is_success() {
        Ok(())
    } else {
        Err("one or more hardware checks failed".to_owned())
    }
}
