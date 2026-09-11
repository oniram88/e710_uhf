use crate::hil::HilOptions;
use std::time::Duration;

pub enum Endpoint {
    Rs232(String),
    Tcp(String),
}

pub struct Config {
    pub endpoint: Endpoint,
    pub options: HilOptions,
}

pub fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Config, String> {
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

pub fn usage(example: &str) -> String {
    format!(
        "Usage:\n  cargo run --example {example}{} -- rs232 <device> [options]\n  cargo run --example {example}{} -- tcp <host:port> [options]\n\nOptions:\n  --timeout-ms <ms>       I/O and response timeout (default: 1000)\n  --antenna               Scan all antenna ports and check VSWR\n  --reference-mhz <MHz>   VSWR fallback frequency (default: 866.0)\n  --inventory             Run one finite inventory round (RF is activated)\n  --require-tag           Fail if the inventory finds no tags\n  --write-checks          Rewrite values read from the reader and check ACKs\n  -h, --help              Show this help",
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
