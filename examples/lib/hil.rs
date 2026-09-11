#![allow(dead_code)] // Shared by the sync and async example binaries.

use e710_uhf::connector::sync::SyncIO;
use e710_uhf::connector::{Connector, ConnectorError};
use e710_uhf::frame::FrameError;
use e710_uhf::frame::command::{
    Command, CommandResult, PhaseStatus, RfLinkProfile, Session, Target,
};
use e710_uhf::frequency_references::Spectrum;
use std::fmt::Display;
use std::io::{Read, Write};
use std::time::Duration;

const MAX_ANTENNA_INDEX: u8 = 7;
const MAX_OUTPUT_POWER_DBM: u8 = 33;

#[derive(Clone, Debug)]
pub struct HilOptions {
    pub timeout: Duration,
    pub check_antenna: bool,
    pub inventory: bool,
    pub require_tag: bool,
    pub write_checks: bool,
    pub fallback_reference_mhz: f64,
}

impl Default for HilOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_millis(1_000),
            check_antenna: false,
            inventory: false,
            require_tag: false,
            write_checks: false,
            fallback_reference_mhz: 866.0,
        }
    }
}

#[derive(Default)]
pub struct HilReport {
    passed: usize,
    failed: usize,
    skipped: usize,
}

impl HilReport {
    pub fn is_success(&self) -> bool {
        self.failed == 0
    }

    pub fn print_summary(&self) {
        println!(
            "\nHIL result: {} passed, {} failed, {} skipped",
            self.passed, self.failed, self.skipped
        );
    }

    fn pass(&mut self, name: &str, detail: impl Display) {
        self.passed += 1;
        println!("[PASS] {name}: {detail}");
    }

    fn fail(&mut self, name: &str, detail: impl Display) {
        self.failed += 1;
        eprintln!("[FAIL] {name}: {detail}");
    }

    fn skip(&mut self, name: &str, reason: impl Display) {
        self.skipped += 1;
        println!("[SKIP] {name}: {reason}");
    }
}

#[cfg(feature = "async")]
pub async fn run_async<S>(stream: S, options: &HilOptions) -> HilReport
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send,
{
    let mut connector = Connector::new(
        stream,
        8,
        vec![25],
        (Spectrum::ETSI, 865.0, 868.0),
        Some(options.timeout),
    );
    let mut report = HilReport::default();

    check_async(
        &mut connector,
        &mut report,
        "firmware",
        Command::GetFirmwareVersion,
        |response| match response {
            CommandResult::GetFirmwareVersion(Ok((major, minor))) => {
                Ok(((major, minor), format!("version {major}.{minor}")))
            }
            CommandResult::GetFirmwareVersion(Err(error)) => Err(error.to_string()),
            other => Err(unexpected_response(&other)),
        },
    )
    .await;

    let antenna = check_async(
        &mut connector,
        &mut report,
        "working antenna",
        Command::GetWorkAntenna,
        |response| match response {
            CommandResult::GetWorkAntenna(Ok(index)) if index <= MAX_ANTENNA_INDEX => {
                Ok((index, format!("zero-based index {index}")))
            }
            CommandResult::GetWorkAntenna(Ok(index)) => Err(format!(
                "index {index} outside the supported range 0..={MAX_ANTENNA_INDEX}"
            )),
            CommandResult::GetWorkAntenna(Err(error)) => Err(error.to_string()),
            other => Err(unexpected_response(&other)),
        },
    )
    .await;

    let powers = check_async(
        &mut connector,
        &mut report,
        "output power",
        Command::GetOutputPower,
        |response| match response {
            CommandResult::GetOutputPower(Ok(values))
                if !values.is_empty()
                    && values.iter().all(|value| *value <= MAX_OUTPUT_POWER_DBM) =>
            {
                let detail = format!("{values:?} dBm");
                Ok((values, detail))
            }
            CommandResult::GetOutputPower(Ok(values)) => Err(format!(
                "expected one or more values in 0..={MAX_OUTPUT_POWER_DBM}, got {values:?}"
            )),
            CommandResult::GetOutputPower(Err(error)) => Err(error.to_string()),
            other => Err(unexpected_response(&other)),
        },
    )
    .await;

    let frequency_region = check_async(
        &mut connector,
        &mut report,
        "frequency region",
        Command::GetFrequencyRegion,
        |response| match response {
            CommandResult::GetFrequencyRegion(Ok((spectrum, min, max))) if min <= max => {
                let detail = format!("{spectrum}, {min:.3}..={max:.3} MHz");
                Ok(((spectrum, min, max), detail))
            }
            CommandResult::GetFrequencyRegion(Ok((spectrum, min, max))) => Err(format!(
                "invalid {spectrum} range: minimum {min} is greater than maximum {max}"
            )),
            CommandResult::GetFrequencyRegion(Err(error)) => Err(error.to_string()),
            other => Err(unexpected_response(&other)),
        },
    )
    .await;

    check_async(
        &mut connector,
        &mut report,
        "reader temperature",
        Command::GetReaderTemperature,
        |response| match response {
            CommandResult::GetReaderTemperature(Ok(value)) if value.is_finite() => {
                Ok((value, format!("{value:.1} °C")))
            }
            CommandResult::GetReaderTemperature(Ok(value)) => {
                Err(format!("non-finite temperature: {value}"))
            }
            CommandResult::GetReaderTemperature(Err(error)) => Err(error.to_string()),
            other => Err(unexpected_response(&other)),
        },
    )
    .await;

    let detector = check_async(
        &mut connector,
        &mut report,
        "antenna detector",
        Command::GetAntConnectionDetector,
        |response| match response {
            CommandResult::GetAntConnectionDetector(Ok(value)) => {
                Ok((value, format!("sensitivity 0x{value:02X}")))
            }
            CommandResult::GetAntConnectionDetector(Err(error)) => Err(error.to_string()),
            other => Err(unexpected_response(&other)),
        },
    )
    .await;

    let rf_profile = check_async(
        &mut connector,
        &mut report,
        "RF link profile",
        Command::GetRfLinkProfile,
        |response| match response {
            CommandResult::GetRfLinkProfile(Ok(profile)) => {
                let detail = format!("{profile:?}");
                Ok((profile, detail))
            }
            CommandResult::GetRfLinkProfile(Err(error)) => Err(error.to_string()),
            other => Err(unexpected_response(&other)),
        },
    )
    .await;

    let connected_antennas = if options.check_antenna {
        let reference_mhz = frequency_region
            .as_ref()
            .map(|(_, min, max)| ((min + max) / 2.0).trunc())
            .unwrap_or(options.fallback_reference_mhz);

        Some(scan_antennas_async(&mut connector, &mut report, antenna, reference_mhz).await)
    } else {
        report.skip("antenna scan", "enable with --antenna");
        None
    };

    if options.write_checks {
        run_write_checks_async(
            &mut connector,
            &mut report,
            antenna,
            powers,
            frequency_region,
            detector,
            rf_profile,
        )
        .await;
    } else {
        report.skip("idempotent write checks", "enable with --write-checks");
    }

    if options.inventory || options.require_tag {
        let inventory_command = match connected_antennas.as_ref() {
            Some(antennas) if antennas.is_empty() => {
                report.skip(
                    "single inventory round",
                    "antenna scan found no connected ports",
                );
                return report;
            }
            Some(antennas) => Command::FastSwitchAntInventory(
                antennas
                    .iter()
                    .map(|(antenna, _vswr)| (*antenna, 1))
                    .collect(),
                0,
                Session::S1,
                Target::A,
                PhaseStatus::Off,
                1,
            ),
            None => Command::CustomizeSessionTargetInventory(
                Session::S1,
                Target::A,
                PhaseStatus::Off,
                1,
            ),
        };

        check_async(
            &mut connector,
            &mut report,
            "single inventory round",
            inventory_command,
            |response| match response {
                CommandResult::ResponsePackets(Ok((tags, result)))
                    if !options.require_tag || !tags.is_empty() =>
                {
                    let count = tags.len();
                    Ok((
                        count,
                        format!(
                            "{count} tag(s), total_read={}, read_rate={}",
                            result.total_read, result.read_rate
                        ),
                    ))
                }
                CommandResult::ResponsePackets(Ok((_tags, _result))) => {
                    Err("no tag detected; --require-tag requires at least one".to_owned())
                }
                CommandResult::ResponsePackets(Err(error)) => Err(error.to_string()),
                other => Err(unexpected_response(&other)),
            },
        )
        .await;
    } else {
        report.skip("single inventory round", "enable with --inventory");
    }

    report
}

pub fn run<S>(stream: S, options: &HilOptions) -> HilReport
where
    S: Read + Write,
{
    // Questi valori sono richiesti dal costruttore, ma il profilo HIL non chiama
    // setup_reader: legge prima la configurazione effettiva dall'hardware.
    let mut connector = Connector::new(
        stream,
        8,
        vec![25],
        (Spectrum::ETSI, 865.0, 868.0),
        Some(options.timeout),
    );
    let mut report = HilReport::default();

    check(
        &mut connector,
        &mut report,
        "firmware",
        Command::GetFirmwareVersion,
        |response| match response {
            CommandResult::GetFirmwareVersion(Ok((major, minor))) => {
                Ok(((major, minor), format!("version {major}.{minor}")))
            }
            CommandResult::GetFirmwareVersion(Err(error)) => Err(error.to_string()),
            other => Err(unexpected_response(&other)),
        },
    );

    let antenna = check(
        &mut connector,
        &mut report,
        "working antenna",
        Command::GetWorkAntenna,
        |response| match response {
            CommandResult::GetWorkAntenna(Ok(index)) if index <= MAX_ANTENNA_INDEX => {
                Ok((index, format!("zero-based index {index}")))
            }
            CommandResult::GetWorkAntenna(Ok(index)) => Err(format!(
                "index {index} outside the supported range 0..={MAX_ANTENNA_INDEX}"
            )),
            CommandResult::GetWorkAntenna(Err(error)) => Err(error.to_string()),
            other => Err(unexpected_response(&other)),
        },
    );

    let powers = check(
        &mut connector,
        &mut report,
        "output power",
        Command::GetOutputPower,
        |response| match response {
            CommandResult::GetOutputPower(Ok(values))
                if !values.is_empty()
                    && values.iter().all(|value| *value <= MAX_OUTPUT_POWER_DBM) =>
            {
                let detail = format!("{values:?} dBm");
                Ok((values, detail))
            }
            CommandResult::GetOutputPower(Ok(values)) => Err(format!(
                "expected one or more values in 0..={MAX_OUTPUT_POWER_DBM}, got {values:?}"
            )),
            CommandResult::GetOutputPower(Err(error)) => Err(error.to_string()),
            other => Err(unexpected_response(&other)),
        },
    );

    let frequency_region = check(
        &mut connector,
        &mut report,
        "frequency region",
        Command::GetFrequencyRegion,
        |response| match response {
            CommandResult::GetFrequencyRegion(Ok((spectrum, min, max))) if min <= max => {
                let detail = format!("{spectrum}, {min:.3}..={max:.3} MHz");
                Ok(((spectrum, min, max), detail))
            }
            CommandResult::GetFrequencyRegion(Ok((spectrum, min, max))) => Err(format!(
                "invalid {spectrum} range: minimum {min} is greater than maximum {max}"
            )),
            CommandResult::GetFrequencyRegion(Err(error)) => Err(error.to_string()),
            other => Err(unexpected_response(&other)),
        },
    );

    check(
        &mut connector,
        &mut report,
        "reader temperature",
        Command::GetReaderTemperature,
        |response| match response {
            CommandResult::GetReaderTemperature(Ok(value)) if value.is_finite() => {
                Ok((value, format!("{value:.1} °C")))
            }
            CommandResult::GetReaderTemperature(Ok(value)) => {
                Err(format!("non-finite temperature: {value}"))
            }
            CommandResult::GetReaderTemperature(Err(error)) => Err(error.to_string()),
            other => Err(unexpected_response(&other)),
        },
    );

    let detector = check(
        &mut connector,
        &mut report,
        "antenna detector",
        Command::GetAntConnectionDetector,
        |response| match response {
            CommandResult::GetAntConnectionDetector(Ok(value)) => {
                Ok((value, format!("sensitivity 0x{value:02X}")))
            }
            CommandResult::GetAntConnectionDetector(Err(error)) => Err(error.to_string()),
            other => Err(unexpected_response(&other)),
        },
    );

    let rf_profile = check(
        &mut connector,
        &mut report,
        "RF link profile",
        Command::GetRfLinkProfile,
        |response| match response {
            CommandResult::GetRfLinkProfile(Ok(profile)) => {
                let detail = format!("{profile:?}");
                Ok((profile, detail))
            }
            CommandResult::GetRfLinkProfile(Err(error)) => Err(error.to_string()),
            other => Err(unexpected_response(&other)),
        },
    );

    let connected_antennas = if options.check_antenna {
        let reference_mhz = frequency_region
            .as_ref()
            .map(|(_, min, max)| ((min + max) / 2.0).trunc())
            .unwrap_or(options.fallback_reference_mhz);

        Some(scan_antennas(
            &mut connector,
            &mut report,
            antenna,
            reference_mhz,
        ))
    } else {
        report.skip("antenna scan", "enable with --antenna");
        None
    };

    if options.write_checks {
        run_write_checks(
            &mut connector,
            &mut report,
            antenna,
            powers,
            frequency_region,
            detector,
            rf_profile,
        );
    } else {
        report.skip("idempotent write checks", "enable with --write-checks");
    }

    if options.inventory || options.require_tag {
        let inventory_command = match connected_antennas.as_ref() {
            Some(antennas) if antennas.is_empty() => {
                report.skip(
                    "single inventory round",
                    "antenna scan found no connected ports",
                );
                return report;
            }
            Some(antennas) => Command::FastSwitchAntInventory(
                antennas
                    .iter()
                    .map(|(antenna, _vswr)| (*antenna, 1))
                    .collect(),
                0,
                Session::S1,
                Target::A,
                PhaseStatus::Off,
                1,
            ),
            None => Command::CustomizeSessionTargetInventory(
                Session::S1,
                Target::A,
                PhaseStatus::Off,
                1,
            ),
        };

        check(
            &mut connector,
            &mut report,
            "single inventory round",
            inventory_command,
            |response| match response {
                CommandResult::ResponsePackets(Ok((tags, result)))
                    if !options.require_tag || !tags.is_empty() =>
                {
                    let count = tags.len();
                    Ok((
                        count,
                        format!(
                            "{count} tag(s), total_read={}, read_rate={}",
                            result.total_read, result.read_rate
                        ),
                    ))
                }
                CommandResult::ResponsePackets(Ok((_tags, _result))) => {
                    Err("no tag detected; --require-tag requires at least one".to_owned())
                }
                CommandResult::ResponsePackets(Err(error)) => Err(error.to_string()),
                other => Err(unexpected_response(&other)),
            },
        );
    } else {
        report.skip("single inventory round", "enable with --inventory");
    }

    report
}

fn scan_antennas<S>(
    connector: &mut Connector<S>,
    report: &mut HilReport,
    original_antenna: Option<u8>,
    reference_mhz: f64,
) -> Vec<(u8, f64)>
where
    S: Read + Write,
{
    let mut connected = Vec::new();
    let mut errors = Vec::new();

    for antenna in 0..=MAX_ANTENNA_INDEX {
        if let Err(error) = set_work_antenna(connector, antenna) {
            errors.push(format!("port {antenna}: {error}"));
            continue;
        }

        match connector.send_and_read_command(Command::GetRfPortReturnLoss(reference_mhz)) {
            Ok(CommandResult::GetRfPortReturnLoss(Ok(vswr))) if vswr.is_finite() && vswr >= 1.0 => {
                println!(
                    "[INFO] antenna {antenna}: connected, VSWR {vswr:.2} at {reference_mhz:.3} MHz"
                );
                connected.push((antenna, vswr));
            }
            Ok(CommandResult::GetRfPortReturnLoss(Ok(vswr))) => {
                errors.push(format!("port {antenna}: invalid VSWR {vswr}"));
            }
            Ok(CommandResult::GetRfPortReturnLoss(Err(FrameError::AntennaNotConnected))) => {
                println!("[INFO] antenna {antenna}: not connected");
            }
            Ok(CommandResult::GetRfPortReturnLoss(Err(error))) => {
                errors.push(format!("port {antenna}: {error}"));
            }
            Ok(other) => errors.push(format!("port {antenna}: {}", unexpected_response(&other))),
            Err(error) => errors.push(format!("port {antenna}: {}", connector_error(&error))),
        }
    }

    if let Some(original_antenna) = original_antenna
        && let Err(error) = set_work_antenna(connector, original_antenna)
    {
        errors.push(format!(
            "cannot restore original port {original_antenna}: {error}"
        ));
    }

    finish_antenna_scan(report, reference_mhz, connected, errors)
}

fn finish_antenna_scan(
    report: &mut HilReport,
    reference_mhz: f64,
    connected: Vec<(u8, f64)>,
    errors: Vec<String>,
) -> Vec<(u8, f64)> {
    if !errors.is_empty() {
        report.fail("antenna scan", errors.join("; "));
    } else if connected.is_empty() {
        report.fail(
            "antenna scan",
            format!("no connected antenna found at {reference_mhz:.3} MHz"),
        );
    } else {
        let detail = connected
            .iter()
            .map(|(antenna, vswr)| format!("port {antenna} (VSWR {vswr:.2})"))
            .collect::<Vec<_>>()
            .join(", ");
        report.pass("antenna scan", detail);
    }

    connected
}

fn set_work_antenna<S>(connector: &mut Connector<S>, antenna: u8) -> Result<(), String>
where
    S: Read + Write,
{
    match connector.send_and_read_command(Command::SetWorkAntenna(antenna)) {
        Ok(CommandResult::SetWorkAntenna(Ok(()))) => Ok(()),
        Ok(CommandResult::SetWorkAntenna(Err(error))) => Err(error.to_string()),
        Ok(other) => Err(unexpected_response(&other)),
        Err(error) => Err(connector_error(&error)),
    }
}

#[allow(clippy::too_many_arguments)]
fn run_write_checks<S>(
    connector: &mut Connector<S>,
    report: &mut HilReport,
    antenna: Option<u8>,
    powers: Option<Vec<u8>>,
    frequency_region: Option<(Spectrum, f64, f64)>,
    detector: Option<u8>,
    rf_profile: Option<RfLinkProfile>,
) where
    S: Read + Write,
{
    if let Some(antenna) = antenna {
        check_unit_response(
            connector,
            report,
            "write current antenna",
            Command::SetWorkAntenna(antenna),
            |response| match response {
                CommandResult::SetWorkAntenna(result) => result.map_err(|error| error.to_string()),
                other => Err(unexpected_response(&other)),
            },
        );
    } else {
        report.skip("write current antenna", "read prerequisite failed");
    }

    if let Some(powers) = powers {
        check_unit_response(
            connector,
            report,
            "write current output power",
            Command::SetOutputPower(powers),
            |response| match response {
                CommandResult::SetOutputPower(result) => result.map_err(|error| error.to_string()),
                other => Err(unexpected_response(&other)),
            },
        );
    } else {
        report.skip("write current output power", "read prerequisite failed");
    }

    if let Some((spectrum, min, max)) = frequency_region {
        check_unit_response(
            connector,
            report,
            "write current frequency region",
            Command::SetDefaultFrequencyRegion(spectrum, min, max),
            |response| match response {
                CommandResult::SetDefaultFrequencyRegion(result) => {
                    result.map_err(|error| error.to_string())
                }
                other => Err(unexpected_response(&other)),
            },
        );
    } else {
        report.skip("write current frequency region", "read prerequisite failed");
    }

    if let Some(detector) = detector {
        check_unit_response(
            connector,
            report,
            "write current antenna detector",
            Command::SetAntConnectionDetector(detector),
            |response| match response {
                CommandResult::SetAntConnectionDetector(result) => {
                    result.map_err(|error| error.to_string())
                }
                other => Err(unexpected_response(&other)),
            },
        );
    } else {
        report.skip("write current antenna detector", "read prerequisite failed");
    }

    if let Some(profile) = rf_profile {
        check_unit_response(
            connector,
            report,
            "write current RF link profile",
            Command::SetRfLinkProfile(profile),
            |response| match response {
                CommandResult::SetRfLinkProfile(result) => {
                    result.map_err(|error| error.to_string())
                }
                other => Err(unexpected_response(&other)),
            },
        );
    } else {
        report.skip("write current RF link profile", "read prerequisite failed");
    }
}

fn check_unit_response<S>(
    connector: &mut Connector<S>,
    report: &mut HilReport,
    name: &str,
    command: Command,
    extract: impl FnOnce(CommandResult) -> Result<(), String>,
) where
    S: Read + Write,
{
    check(connector, report, name, command, |response| {
        extract(response).map(|()| ((), "acknowledged".to_owned()))
    });
}

fn check<S, T>(
    connector: &mut Connector<S>,
    report: &mut HilReport,
    name: &str,
    command: Command,
    validate: impl FnOnce(CommandResult) -> Result<(T, String), String>,
) -> Option<T>
where
    S: Read + Write,
{
    match connector.send_and_read_command(command) {
        Ok(response) => match validate(response) {
            Ok((value, detail)) => {
                report.pass(name, detail);
                Some(value)
            }
            Err(error) => {
                report.fail(name, error);
                None
            }
        },
        Err(error) => {
            report.fail(name, connector_error(&error));
            None
        }
    }
}

fn connector_error(error: &ConnectorError) -> String {
    format!("transport or protocol error: {error}")
}

fn unexpected_response(response: &CommandResult) -> String {
    format!("unexpected response variant: {response:?}")
}

#[cfg(feature = "async")]
async fn scan_antennas_async<S>(
    connector: &mut Connector<S>,
    report: &mut HilReport,
    original_antenna: Option<u8>,
    reference_mhz: f64,
) -> Vec<(u8, f64)>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send,
{
    use e710_uhf::connector::AsyncIO;

    let mut connected = Vec::new();
    let mut errors = Vec::new();

    for antenna in 0..=MAX_ANTENNA_INDEX {
        if let Err(error) = set_work_antenna_async(connector, antenna).await {
            errors.push(format!("port {antenna}: {error}"));
            continue;
        }

        match connector
            .send_and_read_command(Command::GetRfPortReturnLoss(reference_mhz))
            .await
        {
            Ok(CommandResult::GetRfPortReturnLoss(Ok(vswr))) if vswr.is_finite() && vswr >= 1.0 => {
                println!(
                    "[INFO] antenna {antenna}: connected, VSWR {vswr:.2} at {reference_mhz:.3} MHz"
                );
                connected.push((antenna, vswr));
            }
            Ok(CommandResult::GetRfPortReturnLoss(Ok(vswr))) => {
                errors.push(format!("port {antenna}: invalid VSWR {vswr}"));
            }
            Ok(CommandResult::GetRfPortReturnLoss(Err(FrameError::AntennaNotConnected))) => {
                println!("[INFO] antenna {antenna}: not connected");
            }
            Ok(CommandResult::GetRfPortReturnLoss(Err(error))) => {
                errors.push(format!("port {antenna}: {error}"));
            }
            Ok(other) => errors.push(format!("port {antenna}: {}", unexpected_response(&other))),
            Err(error) => errors.push(format!("port {antenna}: {}", connector_error(&error))),
        }
    }

    if let Some(original_antenna) = original_antenna
        && let Err(error) = set_work_antenna_async(connector, original_antenna).await
    {
        errors.push(format!(
            "cannot restore original port {original_antenna}: {error}"
        ));
    }

    finish_antenna_scan(report, reference_mhz, connected, errors)
}

#[cfg(feature = "async")]
async fn set_work_antenna_async<S>(connector: &mut Connector<S>, antenna: u8) -> Result<(), String>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send,
{
    use e710_uhf::connector::AsyncIO;

    match connector
        .send_and_read_command(Command::SetWorkAntenna(antenna))
        .await
    {
        Ok(CommandResult::SetWorkAntenna(Ok(()))) => Ok(()),
        Ok(CommandResult::SetWorkAntenna(Err(error))) => Err(error.to_string()),
        Ok(other) => Err(unexpected_response(&other)),
        Err(error) => Err(connector_error(&error)),
    }
}

#[cfg(feature = "async")]
#[allow(clippy::too_many_arguments)]
async fn run_write_checks_async<S>(
    connector: &mut Connector<S>,
    report: &mut HilReport,
    antenna: Option<u8>,
    powers: Option<Vec<u8>>,
    frequency_region: Option<(Spectrum, f64, f64)>,
    detector: Option<u8>,
    rf_profile: Option<RfLinkProfile>,
) where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send,
{
    if let Some(antenna) = antenna {
        check_unit_response_async(
            connector,
            report,
            "write current antenna",
            Command::SetWorkAntenna(antenna),
            |response| match response {
                CommandResult::SetWorkAntenna(result) => result.map_err(|error| error.to_string()),
                other => Err(unexpected_response(&other)),
            },
        )
        .await;
    } else {
        report.skip("write current antenna", "read prerequisite failed");
    }

    if let Some(powers) = powers {
        check_unit_response_async(
            connector,
            report,
            "write current output power",
            Command::SetOutputPower(powers),
            |response| match response {
                CommandResult::SetOutputPower(result) => result.map_err(|error| error.to_string()),
                other => Err(unexpected_response(&other)),
            },
        )
        .await;
    } else {
        report.skip("write current output power", "read prerequisite failed");
    }

    if let Some((spectrum, min, max)) = frequency_region {
        check_unit_response_async(
            connector,
            report,
            "write current frequency region",
            Command::SetDefaultFrequencyRegion(spectrum, min, max),
            |response| match response {
                CommandResult::SetDefaultFrequencyRegion(result) => {
                    result.map_err(|error| error.to_string())
                }
                other => Err(unexpected_response(&other)),
            },
        )
        .await;
    } else {
        report.skip("write current frequency region", "read prerequisite failed");
    }

    if let Some(detector) = detector {
        check_unit_response_async(
            connector,
            report,
            "write current antenna detector",
            Command::SetAntConnectionDetector(detector),
            |response| match response {
                CommandResult::SetAntConnectionDetector(result) => {
                    result.map_err(|error| error.to_string())
                }
                other => Err(unexpected_response(&other)),
            },
        )
        .await;
    } else {
        report.skip("write current antenna detector", "read prerequisite failed");
    }

    if let Some(profile) = rf_profile {
        check_unit_response_async(
            connector,
            report,
            "write current RF link profile",
            Command::SetRfLinkProfile(profile),
            |response| match response {
                CommandResult::SetRfLinkProfile(result) => {
                    result.map_err(|error| error.to_string())
                }
                other => Err(unexpected_response(&other)),
            },
        )
        .await;
    } else {
        report.skip("write current RF link profile", "read prerequisite failed");
    }
}

#[cfg(feature = "async")]
async fn check_unit_response_async<S>(
    connector: &mut Connector<S>,
    report: &mut HilReport,
    name: &str,
    command: Command,
    extract: impl FnOnce(CommandResult) -> Result<(), String>,
) where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send,
{
    check_async(connector, report, name, command, |response| {
        extract(response).map(|()| ((), "acknowledged".to_owned()))
    })
    .await;
}

#[cfg(feature = "async")]
async fn check_async<S, T>(
    connector: &mut Connector<S>,
    report: &mut HilReport,
    name: &str,
    command: Command,
    validate: impl FnOnce(CommandResult) -> Result<(T, String), String>,
) -> Option<T>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send,
{
    use e710_uhf::connector::AsyncIO;

    match connector.send_and_read_command(command).await {
        Ok(response) => match validate(response) {
            Ok((value, detail)) => {
                report.pass(name, detail);
                Some(value)
            }
            Err(error) => {
                report.fail(name, error);
                None
            }
        },
        Err(error) => {
            report.fail(name, connector_error(&error));
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::io;

    struct ScriptedReader {
        responses: VecDeque<Vec<u8>>,
    }

    impl ScriptedReader {
        fn healthy() -> Self {
            Self {
                responses: VecDeque::from([
                    response(0x72, &[1, 2]),
                    response(0x75, &[0]),
                    response(0x77, &[25]),
                    response(0x79, &[0x02, 0x00, 0x06]),
                    response(0x7B, &[1, 25]),
                    response(0x63, &[3]),
                    response(0x6A, &[0xD1]),
                ]),
            }
        }
    }

    impl Read for ScriptedReader {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            let Some(response) = self.responses.pop_front() else {
                return Ok(0);
            };
            let length = response.len().min(buffer.len());
            buffer[..length].copy_from_slice(&response[..length]);
            Ok(length)
        }
    }

    impl Write for ScriptedReader {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn default_hil_profile_passes_with_valid_reader_responses() {
        let report = run(ScriptedReader::healthy(), &HilOptions::default());

        assert!(report.is_success());
        assert_eq!(report.passed, 7);
        assert_eq!(report.failed, 0);
        assert_eq!(report.skipped, 3);
    }

    #[test]
    fn invalid_hardware_value_fails_the_hil_report() {
        let mut reader = ScriptedReader::healthy();
        reader.responses[2] = response(0x77, &[MAX_OUTPUT_POWER_DBM + 1]);

        let report = run(reader, &HilOptions::default());

        assert!(!report.is_success());
        assert_eq!(report.failed, 1);
    }

    fn response(command: u8, data: &[u8]) -> Vec<u8> {
        let mut frame = vec![0xA0, (data.len() + 3) as u8, 0x01, command];
        frame.extend_from_slice(data);
        let sum = frame
            .iter()
            .fold(0_u8, |sum, value| sum.wrapping_add(*value));
        frame.push((!sum).wrapping_add(1));
        frame
    }
}
