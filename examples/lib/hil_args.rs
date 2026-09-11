use crate::hil::HilOptions;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug)]
pub enum Endpoint {
    Rs232(String),
    Tcp(String),
}

#[derive(Debug)]
pub struct Config {
    pub endpoint: Endpoint,
    pub options: HilOptions,
}

pub fn parse_args(
    args: impl IntoIterator<Item = String>,
    async_inventory: bool,
) -> Result<Config, String> {
    let mut args = args.into_iter();
    let transport = args.next().ok_or_else(|| "missing transport".to_owned())?;
    let target = args
        .next()
        .ok_or_else(|| format!("missing target for transport {transport}"))?;
    let endpoint = match transport.as_str() {
        "rs232" | "serial" => Endpoint::Rs232(target),
        "tcp" => Endpoint::Tcp(target),
        _ => return Err(format!("unsupported transport: {transport}")),
    };
    let mut options = HilOptions::default();

    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--antenna" => options.check_antenna = true,
            "--inventory" => options.inventory = true,
            "--require-tag" => {
                options.inventory = true;
                options.require_tag = true;
            }
            "--write-checks" => options.write_checks = true,
            "--duration-secs" => {
                require_async_option(async_inventory, "--duration-secs")?;
                let value = args
                    .next()
                    .ok_or_else(|| "--duration-secs requires a value".to_owned())?;
                let seconds = value
                    .parse::<u64>()
                    .map_err(|error| format!("invalid inventory duration {value}: {error}"))?;
                if seconds == 0 {
                    return Err("--duration-secs must be greater than zero".to_owned());
                }
                options.inventory_duration = Duration::from_secs(seconds);
            }
            "--output" => {
                require_async_option(async_inventory, "--output")?;
                let value = args
                    .next()
                    .ok_or_else(|| "--output requires a path".to_owned())?;
                options.inventory_output = Some(PathBuf::from(value));
            }
            "--buffer-capacity" => {
                require_async_option(async_inventory, "--buffer-capacity")?;
                let value = args
                    .next()
                    .ok_or_else(|| "--buffer-capacity requires a value".to_owned())?;
                let capacity = value
                    .parse::<usize>()
                    .map_err(|error| format!("invalid buffer capacity {value}: {error}"))?;
                if capacity == 0 {
                    return Err("--buffer-capacity must be greater than zero".to_owned());
                }
                options.buffer_capacity = capacity;
            }
            "--timeout-ms" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--timeout-ms requires a value".to_owned())?;
                let milliseconds = value
                    .parse::<u64>()
                    .map_err(|error| format!("invalid timeout {value}: {error}"))?;
                if milliseconds == 0 {
                    return Err("--timeout-ms must be greater than zero".to_owned());
                }
                options.timeout = Duration::from_millis(milliseconds);
            }
            "--reference-mhz" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--reference-mhz requires a value".to_owned())?;
                options.fallback_reference_mhz = value
                    .parse::<f64>()
                    .map_err(|error| format!("invalid reference frequency {value}: {error}"))?;
            }
            "--help" | "-h" => return Err("help requested".to_owned()),
            _ => return Err(format!("unknown option: {argument}")),
        }
    }

    Ok(Config { endpoint, options })
}

fn require_async_option(enabled: bool, option: &str) -> Result<(), String> {
    if enabled {
        Ok(())
    } else {
        Err(format!("{option} is only supported by async_hil"))
    }
}

pub fn usage(example: &str) -> String {
    let async_options = if example == "async_hil" {
        "\n  --duration-secs <s>    Continuous inventory duration (default: 120)\n  --output <path>         CSV output (default: timestamped filename)\n  --buffer-capacity <n>   Buffered cycles before backpressure (default: 256)"
    } else {
        ""
    };
    format!(
        "Usage:\n  cargo run --example {example}{} -- rs232 <device> [options]\n  cargo run --example {example}{} -- tcp <host:port> [options]\n\nOptions:\n  --timeout-ms <ms>       I/O and response timeout (default: 1000)\n  --antenna               Scan all antenna ports and check VSWR\n  --reference-mhz <MHz>   VSWR fallback frequency (default: 866.0)\n  --inventory             Run inventory (continuous in async HIL)\n  --require-tag           Fail if the inventory finds no tags\n  --write-checks          Rewrite values read from the reader and check ACKs{async_options}\n  -h, --help              Show this help",
        if example == "async_hil" {
            " --features async"
        } else {
            ""
        },
        if example == "async_hil" {
            " --features async"
        } else {
            ""
        }
    )
}

pub fn is_help(args: &[String]) -> bool {
    matches!(args, [argument] if argument == "--help" || argument == "-h")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_async_continuous_inventory_options() {
        let config = parse_args(
            [
                "rs232",
                "/dev/ttyUSB0",
                "--inventory",
                "--duration-secs",
                "30",
                "--output",
                "inventory.csv",
                "--buffer-capacity",
                "64",
            ]
            .map(str::to_owned),
            true,
        )
        .unwrap();

        assert!(config.options.inventory);
        assert_eq!(config.options.inventory_duration, Duration::from_secs(30));
        assert_eq!(
            config.options.inventory_output,
            Some(PathBuf::from("inventory.csv"))
        );
        assert_eq!(config.options.buffer_capacity, 64);
    }

    #[test]
    fn rejects_async_only_options_for_sync_hil() {
        let result = parse_args(
            ["rs232", "/dev/ttyUSB0", "--duration-secs", "30"].map(str::to_owned),
            false,
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("only supported by async_hil"));
    }
}
