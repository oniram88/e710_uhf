# E710 UHF RFID Library

A Rust library for interacting with E710 UHF RFID modules. This crate provides a high-level API for configuring the reader and performing inventory operations (tag reading).

## Features

- **High-level API**: Easy to use `Connector` for managing the reader and its parameters.
- **TCP/IP Support**: Built-in support for Ethernet-based readers using standard TCP streams.
- **Serial Interface Support**: Support for direct serial communication with RS232-based readers.
- **Comprehensive Command Set**:
    - Firmware version retrieval.
    - Antenna management (Set/Get work antenna, connection detection).
    - Power and Frequency configuration (supports FCC, ETSI, CHN, and CUSTOM spectrums).
    - Real-time monitoring: Temperature and VSWR (Return Loss).
    - RF Link Profile selection.
- **Performance Inventory**:
    - Single antenna reading.
    - Fast switching antenna inventory for high-speed tag collection across multiple antennas.
- **Iterator-based API**: Efficiently process tags as they arrive with a clean iterator interface.
- **Async Support**: Asynchronous communication with the reader using `async-std`.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
e710_uhf = { version = "0.4.0", features = ["async"] }
```

## Quick Start

The following example demonstrates how to connect to a reader, set it up, and start a fast switching inventory.

```rust
use e710_uhf::connector::Connector;
use e710_uhf::frame::command::Command;
use e710_uhf::frequency_references::Spectrum;
use std::net::TcpStream;
use std::time::Duration;

fn main() -> std::io::Result<()> {
    // Reader IP and port (usually 4001 for TCP)
    let addr = "192.168.0.178:4001";
    let stream = TcpStream::connect_timeout(&addr.parse().unwrap(), Duration::from_secs(5))?;
    
    // Create connector: 
    // - stream: the TCP connection
    // - total_antennas: 8
    // - output_power: 25 dBm
    // - frequency: ETSI spectrum, from 865.0 to 868.0 MHz
    let mut connector = Connector::new(stream, 8, vec![25], (Spectrum::ETSI, 865.0, 868.0));
    
    // Initialize the reader with the provided settings
    connector.setup_reader().expect("Failed to setup reader");

    // Retrieve firmware version
    let version = connector.send_and_read_command(Command::GetFirmwareVersion).unwrap();
    println!("Firmware Version: {}", version);

    // Build configuration for fast switching between antennas
    // stay_time_multiplier: 1
    let cfgs = connector.build_fast_switching_antenna_cfg(1).unwrap();
    
    // Start inventory
    let mut iter_tag = connector.new_fast_switching_antenna_iterator(cfgs).unwrap();
    
    println!("Starting inventory...");
    while let Some(res) = iter_tag.next() {
        match res {
            Ok(tag) => println!("Read Tag: {}", tag),
            Err(e) => eprintln!("Error reading tag: {:?}", e),
        }
    }

    Ok(())
}
```

## Ethernet Connection Configuration (Linux)

When connecting directly to the module via Ethernet, you may need to manually configure your network interface to match the module's expectations (often 10Mbps Half-Duplex).

```shell
# Configure interface speed and duplex
sudo ethtool -s enp8s0 speed 10 duplex half autoneg off
sudo ifconfig enp8s0 192.168.0.2 netmask 255.255.255.0

# Restart the interface
sudo ip link set enp8s0 down
sudo ip link set enp8s0 up

# Verify state is UP
ip link show enp8s0
ip route show
```

### ARP Table Population

If the reader is not responding, ensure your host has an ARP entry for it. Pinging the device usually resolves this:

```shell
# Check current ARP table
arp -n

# Ping the reader (default IP is often 192.168.0.178)
ping -c 1 192.168.0.178

# Verify the MAC address is now present
arp -n
```

## Hardware-in-the-loop example

The finite HIL example checks the reader firmware, current antenna, output
power, frequency region, temperature, antenna detector and RF link profile. By
default it only sends read-only commands and returns a non-zero exit code if a
check fails.

Checks that scan antenna ports, activate RF or send idempotent configuration
writes are opt-in. When `--antenna` is combined with `--inventory`, the HIL uses
the detected ports for a fast-switch inventory and restores the original work
antenna after the scan:

```shell
# Synchronous HIL over RS-232
cargo run --example hil -- rs232 /dev/ttyUSB0 \
  --antenna --inventory --require-tag --write-checks

# Asynchronous HIL over RS-232
cargo run --example async_hil --features async -- rs232 /dev/ttyUSB0 \
  --antenna --inventory --require-tag --write-checks \
  --duration-secs 120 --output async_hil_inventory.csv
```

In the async HIL, `--inventory` runs continuously for 120 seconds by default.
Use `--duration-secs` to change the duration, `--output` to choose the CSV file,
and `--buffer-capacity` to configure the bounded channel between the inventory
task and the dedicated file-writer thread:

```shell
cargo run --example async_hil --features async -- rs232 /dev/ttyUSB0 \
  --antenna --inventory --duration-secs 30 \
  --output async_hil_inventory.csv --buffer-capacity 256
```

The console reports every completed cycle with tag count, inventory I/O time,
buffer wait, time since the previous repetition, idle gap and total cycle time.
The CSV stores the same timings and one row for every observed tag; cycles with
no tags or protocol errors are retained as rows as well. This makes gaps between
inventory rounds and writer backpressure directly measurable.

TCP is supported by both runners as well:

```shell
cargo run --example hil -- tcp 192.168.0.178:4001
cargo run --example async_hil --features async -- tcp 192.168.0.178:4001
```

Run `cargo run --example hil -- --help` or
`cargo run --example async_hil --features async -- --help` for all options.
Compiling the examples does not require a reader; executing the HIL does require
a reachable E710.

## License

This project is licensed under the MIT License - see the LICENSE file for details.
