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

## License

This project is licensed under the MIT License - see the LICENSE file for details.

