use crate::error_references::ErrorCode;
use crate::frequency_references::{Spectrum, get_frequency, get_param};
use crate::tag::Tag;
use log::debug;
use std::fmt::{Display, Formatter};

const FRAME_HEADER: u8 = 0xA0;
const RS485_ADDRESS: u8 = 0x01;

#[derive(Debug)]
pub enum FrameError {
    InvalidCommand(String),
    ResponseNotExpected(Vec<u8>),
    InvalidPacket(Vec<u8>),
    FailedResponse(ErrorCode, Vec<u8>),
    AntennaNotConnected,
    TagParsingError(Vec<u8>),
    InvalidChecksum,
}

impl Display for FrameError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            FrameError::InvalidCommand(msg) => write!(f, "Invalid command: {}", msg),
            FrameError::ResponseNotExpected(response) => {
                write!(f, "Response not expected [RX] {:02X?}", response)
            }
            FrameError::InvalidPacket(packet) => write!(f, "Invalid packet [RX] {:02X?}", packet),
            FrameError::FailedResponse(code, packet) => write!(
                f,
                "Failed response with code {:?} and DATA {:02X?}",
                code, packet
            ),
            FrameError::AntennaNotConnected => write!(f, "Antenna not connected"),
            FrameError::TagParsingError(tag) => {
                write!(f, "Tag parsing error with raw data: {:#?}", tag)
            }
            FrameError::InvalidChecksum => write!(f, "Invalid checksum"),
        }
    }
}

impl std::error::Error for FrameError {}

/// [RfLinkProfile]
/// | ProfileID | Descrizione                                                            |
/// | --------- | ----------------------------------------------------------------------- |
/// | `0xD0`    | Profile 0: Tari 25µs, **FM0**, 40 kHz                                   |
/// | `0xD1`    | Profile 1: Tari 25µs, **Miller 4**, 250 kHz (**default / consigliato**) |
/// | `0xD2`    | Profile 2: Tari 25µs, **Miller 4**, 300 kHz                             |
/// | `0xD3`    | Profile 3: Tari 6.25µs, **FM0**, 400 kHz                                |
/// 🔸 Frequenza (40KHz / 250KHz / 300KHz / 400KHz)
///
/// È la velocità del link:
/// più alta → più tag/sec
/// più bassa → più stabilità
///
#[derive(Clone, Debug, PartialEq)]
#[repr(u8)]
pub enum RfLinkProfile {
    Tari25usFM0KHz40 = 0xD0,
    Tari25usMiller4KHz250 = 0xD1,
    Tari25usMiller4KHz300 = 0xD2,
    Tari625usFM0KHz400 = 0xD3,
}

impl RfLinkProfile {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            0xD0 => Some(Self::Tari25usFM0KHz40),
            0xD1 => Some(Self::Tari25usMiller4KHz250),
            0xD2 => Some(Self::Tari25usMiller4KHz300),
            0xD3 => Some(Self::Tari625usFM0KHz400),
            _ => None,
        }
    }
}

/// Enumerazione dei parametri per la configurazione della sessione
#[derive(Clone, Debug, PartialEq)]
pub enum Session {
    S0 = 0x00,
    S1 = 0x01,
    S2 = 0x02,
    S3 = 0x03,
}

/// Enumerazione dei parametri per la configurazione del target di lettura
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    A = 0x00,
    B = 0x01,
}

#[derive(Clone)]
pub enum Command {
    Reset,
    // SetUartBaudRate,
    GetFirmwareVersion,
    // SetReaderAddress,
    /// [SetWorkAntenna] Imposta la posizione dell'antenna di lavoro,
    /// 0 index base => antenna 1, posizione 0
    SetWorkAntenna(u8),
    GetWorkAntenna,
    /// [SetOutputPower] Imposta la potenza di output delle antenne,
    /// con un solo valore andremo ad impostare su tutte le antenne la medesima potenza
    /// con più valori ogni antenna avrà la sua potenza distinta
    SetOutputPower(Vec<u8>),
    GetOutputPower,
    SetDefaultFrequencyRegion(Spectrum, f64, f64), // use default frequencies
    GetFrequencyRegion,
    // SetBeeperMode,
    GetReaderTemperature,
    // ReadGpioValue,
    // WriteGpioValue,
    /// [SetAntConnectionDetector]
    /// Imposta il valore per cui in automatico una porta viene disattivata.
    /// con valore 0x00 il check viene disattivato
    /// il valore di default è 0x03
    SetAntConnectionDetector(u8),
    GetAntConnectionDetector,
    // SetTemporaryOutputPower,
    // SetReaderIdentifier,
    // GetReaderIdentifier,
    SetRfLinkProfile(RfLinkProfile),
    GetRfLinkProfile,
    /// [GetRfPortReturnLoss] il valore impostato è la frequenza di controllo, dovrebbe essere
    /// la frequenza centrale di lavoro. ES. EU 865–868 MHz -> frequenza di check 866 MHz
    GetRfPortReturnLoss(f64),
    // SetReaderName,
    //---- ISO18000-6C Commands
    ///
    /// - Session
    /// - Target
    /// - Phase Value; 00 for turn it off; 01 for turn it on. [CODED to 0x00]
    /// - Repeat the inventory with above ant switch sequence.
    CustomizeSessionTargetInventory(
        Session,
        Target,
        u8, // Phase Value; 00 for turn it off; 01 for turn it on. [CODED to 0x00]
        u8, // Repeat the inventory with above ant switch sequence.
    ),
    ///
    /// - Tuple con antenna_id e stay
    /// - Interval: Rest time between switching antennas. During the cause of rest,
    ///   RF output will be canceled, thus power consumption and heat generation are both reduced.
    /// - Session
    /// - Target
    /// - Phase Value; 00 for turn it off; 01 for turn it on. [CODED to 0x00]
    /// - Repeat the inventory with above ant switch sequence.
    FastSwitchAntInventory(
        // Tuple con antenna_id e stay
        Vec<(
            u8, // Working ant (00 – 07). If set this byte above 07 means ignore it.
            u8, // Inventory round for an antenna. Every antenna has this parameter.
        )>,
        u8, //Interval: Rest time between switching antennas. During the cause of rest,
        // RF output will be canceled, thus power consumption and heat generation are both reduced.
        Session,
        Target,
        u8, // Phase Value; 00 for turn it off; 01 for turn it on. [CODED to 0x00]
        u8, // Repeat the inventory with above ant switch sequence.
    ),
    // Read
    // Write
    // Lock
    // Kill
    // SetAccessEPCMatch
    // GetAccessEPCMatch
    // SetImpinjFastTID
    // GetAndSaveImpinjFastTID
    // GetImpinjFastTID
    //--- ISO18000-6B Commands
    // Iso180006BInventory
    // Iso180006BRead
    // Iso180006BWrite
    // Iso180006BLock
    // Iso180006BQueryLock
}

#[derive(Debug)]
pub enum CommandResult {
    Reset(Result<(), FrameError>),
    GetFirmwareVersion(Result<(u8, u8), FrameError>),
    SetWorkAntenna(Result<(), FrameError>),
    GetWorkAntenna(Result<u8, FrameError>), //posizione antenna
    GetReaderTemperature(Result<f64, FrameError>),
    /// [GetOutputPower]
    /// Ritorna l'array della potenza di output delle antenne,
    /// nel caso di singolo risultato vuol dire che sono configurate
    /// tutte allo stesso modo, valori da 0 to 33(0x00 – 0x21)
    GetOutputPower(Result<Vec<u8>, FrameError>),
    SetDefaultFrequencyRegion(Result<(), FrameError>),
    GetFrequencyRegion(Result<(Spectrum, f64, f64), FrameError>),
    SetAntConnectionDetector(Result<(), FrameError>),
    /// [GetAntConnectionDetector]
    /// The result is the Return Loss sensitivity
    /// 0x00 Connection detector is closed.
    /// >= 0x00 The sensitivity of the antenna detector,
    ///         the value is the return loss of the antenna port.
    ///         The unit is dB.
    GetAntConnectionDetector(Result<u8, FrameError>),
    SetRfLinkProfile(Result<(), FrameError>),
    GetRfLinkProfile(Result<RfLinkProfile, FrameError>),
    /// [GetRfPortReturnLoss] è il risultato del calcolo rispetto alla frequenza passata.
    /// il valore di ritorno è il VSWR calcolato dal ReturnLoss ricevuto dal device
    /// VSWR è solo un altro modo di leggere lo stesso fenomeno.
    ///
    /// | Return Loss (dB) | VSWR  | Interpretazione    |
    /// | ---------------- | ----- | ------------------ |
    /// | 0 dB             | ∞     | antenna scollegata |
    /// | 3 dB             | ~6.0  | pessima            |
    /// | 6 dB             | ~3.0  | accettabile        |
    /// | 10 dB            | ~1.9  | buona              |
    /// | 15 dB            | ~1.4  | molto buona        |
    /// | 20 dB            | ~1.22 | eccellente         |
    ///
    ///  📌 In RFID:
    ///
    ///  VSWR ≤ 3 → generalmente OK
    ///  VSWR ≤ 2 → buono
    ///  VSWR > 5 → problemi
    GetRfPortReturnLoss(Result<f64, FrameError>),
    ResponsePackets(Result<(Vec<Tag>, ReadResult), FrameError>),
}

impl Display for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Command::Reset => write!(f, "Reset"),
            Command::SetWorkAntenna(pos) => write!(f, "Set Work Antenna to {}", pos),
            Command::GetWorkAntenna => write!(f, "Work Antenna"),
            Command::GetFirmwareVersion => write!(f, "Firmware Version"),
            Command::GetReaderTemperature => write!(f, "Temperature"),
            Command::SetOutputPower(v) => {
                if v.len() == 1 {
                    write!(f, "Set Output power globaly to {}", v[0])
                } else {
                    write!(f, "Set Output power for single antenna to {:?}", v)
                }
            }
            Command::GetOutputPower => write!(f, "Output Power"),
            Command::SetDefaultFrequencyRegion(spectrum, min, max) => {
                write!(f, "Set {spectrum} Frequency Region [{min} -> {max}]")
            }
            Command::GetFrequencyRegion => write!(f, "Frequency Region"),
            Command::SetAntConnectionDetector(v) => {
                write!(f, "Set Antenna Connection Detector to {v}")
            }
            Command::GetAntConnectionDetector => write!(f, "Antenna Connection Detector"),
            Command::SetRfLinkProfile(profile) => write!(f, "Set RF Link Profile to {:?}", profile),
            Command::GetRfLinkProfile => write!(f, "Get RF Link Profile"),
            Command::GetRfPortReturnLoss(reference_frequency) => {
                write!(
                    f,
                    "Rf Port Return Loss setted with reference frequency of: {reference_frequency}"
                )
            }
            Command::CustomizeSessionTargetInventory(session, target, phase, tag_count) => write!(
                f,
                "Customize Session Target Inventory for session: {:?} target: {:?} phase: {:?} tag_count: {:?}",
                session, target, phase, tag_count
            ),
            Command::FastSwitchAntInventory(
                antennas,
                rest,
                session,
                target,
                phase,  // Phase Value; 00 for turn it off; 01 for turn it on. [CODED to 0x00]
                repeat, // Repeat the inventory with above ant switch sequence.
            ) => write!(
                f,
                "Fast Switch Antenna Inventory for antennas: {:?} rest: {:?} session: {:?} target: {:?} phase: {:?} repeat: {:?}",
                antennas, rest, session, target, phase, repeat
            ),
        }
    }
}

impl Display for CommandResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandResult::Reset(_) => write!(f, "Failed to reset"), // il reset può solo fallire

            CommandResult::GetFirmwareVersion(Ok((major, minor))) => {
                write!(f, "Firmware Version [{major}.{minor}]")
            }
            CommandResult::GetFirmwareVersion(Err(err)) => {
                write!(f, "Failed to get Firmware Version: {}", err)
            }

            CommandResult::SetWorkAntenna(Ok(())) => {
                write!(f, "Set Working Antenna set successfully")
            }
            CommandResult::SetWorkAntenna(Err(err)) => write!(f, "Failed to set Antenna: {}", err),

            CommandResult::GetWorkAntenna(Ok(pos)) => write!(f, "Work Antenna [{pos}]"),
            CommandResult::GetWorkAntenna(Err(err)) => {
                write!(f, "Failed to get Work Antenna: {}", err)
            }
            CommandResult::GetReaderTemperature(Ok(tmp)) => write!(f, "Temperature [{tmp} °C]"),
            CommandResult::GetReaderTemperature(Err(err)) => {
                write!(f, "Failed to get Temperature: {}", err)
            }
            CommandResult::GetOutputPower(Ok(v)) => write!(f, "Antenna output power {:#?}", v),
            CommandResult::GetOutputPower(Err(e)) => {
                write!(f, "Failed to get Output Antenna Power: {}", e)
            }

            CommandResult::SetDefaultFrequencyRegion(Ok(())) => {
                write!(f, "Frequency Region set successfully")
            }
            CommandResult::SetDefaultFrequencyRegion(Err(err)) => {
                write!(f, "Failed to set Frequency Region: {}", err)
            }
            CommandResult::GetFrequencyRegion(Ok((spectrum, min, max))) => {
                write!(f, "Frequency Region [{spectrum} [{min} -> {max}]")
            }
            CommandResult::GetFrequencyRegion(Err(err)) => {
                write!(f, "Failed to get Frequency Region: {}", err)
            }

            CommandResult::SetAntConnectionDetector(Ok(())) => {
                write!(f, "Antenna Connection Detector set successfully")
            }
            CommandResult::SetAntConnectionDetector(Err(err)) => {
                write!(f, "Failed to set Antenna Connection Detector: {}", err)
            }
            CommandResult::GetAntConnectionDetector(Ok(v)) => {
                if *v == 0x00 {
                    write!(f, "Antenna Connection Detector is closed")
                } else {
                    write!(f, "Antenna Connection Detector [{v}]")
                }
            }
            CommandResult::GetAntConnectionDetector(Err(err)) => {
                write!(f, "Failed to get Connection Detector: {}", err)
            }
            CommandResult::SetRfLinkProfile(Ok(())) => {
                write!(f, "RF Link Profile set successfully")
            }
            CommandResult::SetRfLinkProfile(Err(err)) => {
                write!(f, "Failed to set RF Link Profile: {}", err)
            }
            CommandResult::GetRfLinkProfile(Ok(profile)) => {
                write!(f, "RF Link Profile: {:?}", profile)
            }
            CommandResult::GetRfLinkProfile(Err(err)) => {
                write!(f, "Failed to get RF Link Profile: {}", err)
            }
            CommandResult::GetRfPortReturnLoss(Ok(v)) => {
                write!(
                    f,
                    "Rf Port VSWR[{v}]
                    VSWR > 5 → problems
                    VSWR ≤ 3 → OK
                    VSWR ≤ 2 → good
                    VSWR ≤ 1.5 → very good
                "
                )
            }
            CommandResult::GetRfPortReturnLoss(Err(err)) => {
                write!(f, "Failed to get Rf Port Return Loss: {}", err)
            }

            CommandResult::ResponsePackets(Ok(packets)) => {
                write!(f, "Response Packets: {:#?}", packets)
            }
            CommandResult::ResponsePackets(Err(err)) => {
                write!(f, "Failed to get Response Packets: {}", err)
            }
        }
    }
}

/// Trait for serializable commands
pub(crate) trait SerializableCommand {
    /// Returns a tuple of bytes (command, parameters)
    /// Parameters may be empty if not present
    fn to_bytes(&self) -> Vec<u8>;
    fn from_byte(raw: Vec<u8>) -> Result<CommandResult, FrameError>
    where
        Self: Sized;
}

macro_rules! parse_response {
    ($data:expr) => {
        match ErrorCode::from_hex($data[0]) {
            ErrorCode::CommandSuccess => Ok(()),
            response_error => Err(FrameError::FailedResponse(response_error, $data)),
        }
    };

    ($data:expr, ($min:expr, $max:expr), $success_block:expr) => {
        if $data[0] >= $min && $data[0] <= $max {
            $success_block($data)
        } else {
            parse_response!($data, $success_block)
        }
    };

    ($data:expr, $success_block:expr) => {
        if $data.len() == 1 {
            match ErrorCode::from_hex($data[0]) {
                ErrorCode::CommandSuccess => $success_block($data),
                response_error => Err(FrameError::FailedResponse(response_error, $data)),
            }
        } else {
            $success_block($data)
        }
    };
}

impl SerializableCommand for Command {
    ///
    /// Genera i bytes che identificano comando e dati nel caso di un comando con dati
    fn to_bytes(&self) -> Vec<u8> {
        match self {
            Command::Reset => vec![0x70],
            Command::SetWorkAntenna(index) => {
                vec![0x74, *index]
            }
            Command::GetWorkAntenna => vec![0x75],
            Command::GetFirmwareVersion => vec![0x72],
            Command::GetReaderTemperature => vec![0x7B],
            Command::SetOutputPower(v) => {
                let mut out = vec![0x76];
                out.extend(v);
                out
            }
            Command::GetOutputPower => vec![0x77],
            Command::SetDefaultFrequencyRegion(spectrum, min, max) => {
                let mut v = vec![0x78];
                v.push(spectrum.clone() as u8);
                v.push(get_param(*min));
                v.push(get_param(*max));
                v
            }
            Command::GetFrequencyRegion => vec![0x79],
            Command::SetAntConnectionDetector(v) => {
                vec![0x62, *v]
            }
            Command::GetAntConnectionDetector => vec![0x63],
            Command::SetRfLinkProfile(profile) => {
                vec![0x69, profile.clone() as u8]
            }
            Command::GetRfLinkProfile => vec![0x6A],
            Command::GetRfPortReturnLoss(reference_frequency) => {
                vec![0x7E, get_param(*reference_frequency)]
            }
            Command::CustomizeSessionTargetInventory(session, target, _phase, repeat) => {
                vec![
                    0x8B,
                    session.clone() as u8,
                    target.clone() as u8,
                    0x00, // SL a 0 Select Flag; range from: 00,01,02,03
                    0x00, // Phase Value; 00 for turn it off; 01 for turn it on. [SE usiamo il parametro dobbiam parsare in modo diverso il risultato]
                    repeat.clone(),
                ]
            }
            Command::FastSwitchAntInventory(antennas, interval, session, target, phase, repeat) => {
                let mut v = vec![0x8A];

                let flat: Vec<u8> = antennas.iter().flat_map(|(a, b)| [*a, *b]).collect();

                v.extend(flat);

                // complete remaining antennas spaces
                for _ in 0..(8 - antennas.len()) {
                    v.extend(vec![0x08, 0x00]); // Disabled antenna
                }

                v.push(interval.clone());
                v.extend(vec![0x00, 0x00, 0x00, 0x00, 0x00]); // Reserved bytes
                v.push(session.clone() as u8);
                v.push(target.clone() as u8);
                v.extend(vec![0x00, 0x00, 0x00]); // Reserved bytes
                v.push(phase.clone());
                v.push(repeat.clone());
                v
            }
        }
    }

    ///
    /// Il pacchetto di risposta è dato da
    /// [0] -> Head of the packet, every packet starts with 0xA0.
    /// [1] -> Length of the packet bytes. Starts from the third byte, the Head, Len bytes are exclusive.
    /// [2] -> Reader’s address.
    /// [3] -> Command byte.
    /// [4..] -> Data from the reader.
    /// [length + 2] -> Checksum. Check all the bytes except itself.
    fn from_byte(raw: Vec<u8>) -> Result<CommandResult, FrameError> {
        if raw.len() < 4 {
            return Err(FrameError::InvalidPacket(raw.clone()));
        }

        let (length, raw_command, checksum, data) = split_in_base_frame_parts(&raw);

        if raw_command == 0x8B {
            // In questo caso abbiamo più pacchetti concatenati con più checksums
            Ok(CommandResult::ResponsePackets(parse_response!(
                data,
                |data: Vec<u8>| { Ok(parse_tag_response(raw)?) }
            )))
        } else {
            if checksum != calculate_checksum(&raw[0..(raw.len() - 1)]) {
                return Err(FrameError::InvalidChecksum);
            }

            debug!(
                "CMD[0x{:02X}] DATA[{:?}] CHECKSUM[{}]",
                raw_command, data, checksum
            );

            match raw_command {
                0x62 => Ok(CommandResult::SetAntConnectionDetector(parse_response!(
                    data
                ))),
                0x63 => Ok(CommandResult::GetAntConnectionDetector(Ok(data[0]))),
                0x69 => Ok(CommandResult::SetRfLinkProfile(parse_response!(data))),
                0x6A => Ok(CommandResult::GetRfLinkProfile(parse_response!(
                    data,
                    (0xD0, 0xD3),
                    |data: Vec<u8>| {
                        RfLinkProfile::from_u8(data[0]).ok_or_else(|| {
                            FrameError::InvalidCommand(format!(
                                "Invalid RF link profile: 0x{:02X}",
                                data[0]
                            ))
                        })
                    }
                ))),
                0x70 => Ok(CommandResult::Reset(parse_response!(data))),
                0x72 => Ok(CommandResult::GetFirmwareVersion(Ok((data[0], data[1])))),
                0x74 => Ok(CommandResult::SetWorkAntenna(parse_response!(data))),
                0x75 => Ok(CommandResult::GetWorkAntenna(Ok(data[0] + 1))),
                0x7B => Ok(CommandResult::GetReaderTemperature(parse_response!(
                    data,
                    |data: Vec<u8>| {
                        let sign: f64 = if data[0] == 0x00 { -1.0 } else { 1.0 };
                        Ok(data[1] as f64 * sign)
                    }
                ))),
                0x76 => Ok(CommandResult::Reset(parse_response!(data))),
                0x77 => Ok(CommandResult::GetOutputPower(Ok(data))),
                0x78 => Ok(CommandResult::SetDefaultFrequencyRegion(parse_response!(
                    data
                ))),
                0x79 => {
                    match data[0] {
                        0x01 if length == 6 => Ok(CommandResult::GetFrequencyRegion(
                            parse_response!(data, |data: Vec<u8>| Ok((
                                Spectrum::FCC,
                                get_frequency(data[1]),
                                get_frequency(data[2])
                            ))),
                        )),
                        0x02 if length == 6 => Ok(CommandResult::GetFrequencyRegion(
                            parse_response!(data, |data: Vec<u8>| Ok((
                                Spectrum::ETSI,
                                get_frequency(data[1]),
                                get_frequency(data[2])
                            ))),
                        )),
                        0x03 if length == 6 => Ok(CommandResult::GetFrequencyRegion(
                            parse_response!(data, |data: Vec<u8>| Ok((
                                Spectrum::CHN,
                                get_frequency(data[1]),
                                get_frequency(data[2])
                            ))),
                        )),
                        0x04 if length == 9 => {
                            // todo!("Da completare la versione impostata dall'utente");
                            Ok(CommandResult::GetFrequencyRegion(Ok((
                                Spectrum::CUSTOM,
                                0.0,
                                0.0,
                            ))))
                        }
                        _ => Err(FrameError::ResponseNotExpected(raw.clone())),
                    }
                }
                0x7E => Ok(CommandResult::GetRfPortReturnLoss(parse_response!(
                    data,
                    (0x00, 0x1E),
                    |data: Vec<u8>| {
                        debug!("RF Port Return Loss: {:?}", data);

                        let rl_db = data[0] as f64;
                        if rl_db == 0.0 {
                            Err(FrameError::AntennaNotConnected)
                        } else {
                            let x = 10f64.powf(rl_db / 20.0);
                            let vswr = (x + 1.0) / (x - 1.0);

                            Ok(vswr)
                        }
                    }
                ))),

                _ => Err(FrameError::InvalidCommand(format!(
                    "Invalid Response command code: {}",
                    raw[0]
                ))),
            }
        }
    }
}

fn split_in_base_frame_parts(raw: &[u8]) -> (usize, u8, u8, Vec<u8>) {
    let length = raw[1] as usize;
    let raw_command = raw[3];
    let checksum = raw[length + 1];
    let data = (&raw[4..(4 + length - 3)]).to_vec();
    (length, raw_command, checksum, data)
}

fn from_bytes_to_utf8(bytes: &Vec<u8>) -> String {
    //String::from_utf8_lossy(bytes).to_string()

    if let Ok(text) = std::str::from_utf8(&*bytes) {
        text.to_string()
    } else {
        "Invalid UTF-8".to_string()
    }
}

pub struct Frame {
    payload: Vec<u8>,
}

impl Frame {
    pub fn new(payload: &Command) -> Self {
        Frame {
            payload: payload.to_bytes(),
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v = Vec::new();
        v.push(FRAME_HEADER); // HEADER
        v.push((self.payload.len() + 2) as u8); // payload + un byte per address e un byte per checksum
        //
        // Address for RS-485 Reader’s address for RS-485 connection.
        // The common addresses are 0～254(0xFE)，255（0xFF）is the public address.
        // The reader accepts the address of itself and the public address.
        //
        v.push(RS485_ADDRESS);

        v.extend(&self.payload);

        v.push(calculate_checksum(&v));
        v
    }
}

fn calculate_checksum(buff: &[u8]) -> u8 {
    let mut sum: u8 = 0;

    for &b in buff {
        sum = sum.wrapping_add(b);
    }

    (!sum).wrapping_add(1)
}

#[derive(Debug)]
pub struct ReadResult {
    antenna_id: u8,
    read_rate: u16,
    total_read: u32,
}

fn split_packets(buf: &[u8]) -> Vec<&[u8]> {
    let mut packets = Vec::new();
    let mut offset = 0;

    while offset < buf.len() {
        // cerca header
        if buf[offset] != FRAME_HEADER {
            offset += 1;
            continue;
        }

        // serve almeno Head + Len
        if offset + 2 > buf.len() {
            break;
        }

        let len = buf[offset + 1] as usize;
        let pkt_len = len + 2;

        // pacchetto incompleto
        if offset + pkt_len > buf.len() {
            break;
        }

        packets.push(&buf[offset..offset + pkt_len]);
        offset += pkt_len;
    }

    packets
}

fn parse_tag_response(raw_data: Vec<u8>) -> Result<(Vec<Tag>, ReadResult), FrameError> {
    let mut tags: Vec<Tag> = Vec::new();
    let mut result: ReadResult = ReadResult {
        antenna_id: 0,
        read_rate: 0,
        total_read: 0,
    };

    let packets = split_packets(&raw_data);

    for frame in packets {
        // devo elaborare il pacchetto
        let (length, raw_command, checksum, data) = split_in_base_frame_parts(frame);
        if checksum != calculate_checksum(&frame[0..(frame.len() - 1)]) {
            return Err(FrameError::InvalidChecksum);
        }

        if length == 0x0A {
            // Ultimo frame di check
            result = ReadResult {
                antenna_id: frame[4],
                read_rate: u16::from_be_bytes([frame[5], frame[6]]),
                total_read: u32::from_be_bytes([frame[7], frame[8], frame[9], frame[10]]),
            }
        } else {
            tags.push(Tag::from_raw(&frame[4..frame.len() - 1]));
        }
    }

    Ok((tags, result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tag::Tag;

    #[test]
    fn test_checksum() {
        // A0 03 01 72 (esempio ipotetico, ma usiamo la logica del codice)
        // HEADER = 0xA0
        // LEN = 0x03
        // ADDR = 0x01
        // CMD = 0x72
        // Somma = A0 + 03 + 01 + 72 = 278 (in u8 wrapping)
        // 0xA0 + 0x03 + 0xFF + 0x72 = 0x278
        // !0x278 + 1 = 0xEA
        let data = vec![0xA0, 0x03, 0x01, 0x72];
        assert_eq!(calculate_checksum(&data), 0xEA);
    }

    #[test]
    fn test_command_to_bytes() {
        let cmd = Command::GetFirmwareVersion;
        assert_eq!(cmd.to_bytes(), vec![0x72]);
    }

    #[test]
    fn test_command_from_byte() {
        let cmd = Command::from_byte(vec![0xA0, 0x05, 0x01, 0x72, 0x46, 0x01, 0xA1]).unwrap();
        let expected_version = (70 as u8, 1 as u8);

        assert!(
            matches!(cmd, CommandResult::GetFirmwareVersion(Ok(ref v)) if *v == expected_version)
        );

        let err = Command::from_byte(vec![0x00]);
        assert!(err.is_err());
    }

    #[test]
    fn test_frame_new() {
        let cmd = Command::GetFirmwareVersion;
        let frame = Frame::new(&cmd);
        assert_eq!(frame.payload, vec![0x72]);
    }

    #[test]
    fn test_frame_to_bytes() {
        let cmd = Command::GetFirmwareVersion;
        let frame = Frame::new(&cmd);
        let bytes = frame.to_bytes();

        // HEADER: A0
        // LEN: payload.len() (1) + 2 = 3
        // ADDR: FF
        // PAYLOAD: 72
        // CHECKSUM: EC (calcolato in test_checksum)
        assert_eq!(bytes, vec![0xA0, 0x03, RS485_ADDRESS, 0x72, 0xEA]);
    }

    #[test]
    fn test_frame_to_bytes_fast_switch_ant_inventory() {
        let cmd = Command::FastSwitchAntInventory(
            vec![(0, 1), (1, 1), (6, 1), (7, 1)],
            0,
            Session::S1,
            Target::A,
            0,
            1,
        );

        let bytes = cmd.to_bytes();

        assert_eq!(
            bytes,
            vec![
                0x8A, // tuple antenna + stay
                0x00, 0x01, // antenna 1
                0x01, 0x01, // antenna 2
                0x06, 0x01, // antenna 7
                0x07, 0x01, // antenna 8
                0x08, 0x00, // ignore antenna
                0x08, 0x00, // ignore antenna
                0x08, 0x00, // ignore antenna
                0x08, 0x00, // ignore antenna
                0x00, // interval
                0x00, 0x00, 0x00, 0x00, 0x00, // Reserved bytes
                0x01, // Session
                0x00, // Target
                0x00, 0x00, 0x00, // Reserved bytes
                0x00, // Phase
                0x01  // Repeat
            ]
        );
    }

    #[test]
    fn test_frame_error_display() {
        let err = FrameError::InvalidCommand("test".to_string());
        assert_eq!(format!("{}", err), "Invalid command: test");
    }

    #[test]
    fn test_get_temperature() {
        let cmd = Command::from_byte(vec![0xA0, 0x05, 0x01, 0x7B, 0x01, 0x17, 0xC7]).unwrap();
        let expected = 23.0;

        assert!(matches!(cmd, CommandResult::GetReaderTemperature(Ok(ref v)) if *v == expected));
    }

    #[test]
    fn test_get_frequency_region() {
        let raw_packet = vec![0xA0, 0x06, 0x01, 0x79, 0x01, 0x07, 0x3B, 0x9D];
        let result = Command::from_byte(raw_packet).unwrap();

        if let CommandResult::GetFrequencyRegion(Ok(region)) = result {
            assert_eq!(
                format!("{} {}->{}", region.0, region.1, region.2),
                "FCC 902->928"
            );
        } else {
            panic!("Expected GetFrequencyRegion(Ok), got {:?}", result);
        }
    }

    #[test]
    fn test_get_work_antenna() {
        let raw_packet = vec![0xA0, 0x04, 0x01, 0x75, 0x00, 0xE6];
        let result = Command::from_byte(raw_packet).unwrap();

        if let CommandResult::GetWorkAntenna(Ok(pos)) = result {
            assert_eq!(pos, 1);
        } else {
            panic!("Expected GetWorkAntenna(Ok), got {:?}", result);
        }
    }

    #[test]
    fn test_parse_tag_response() {
        let mut raw_packet = vec![
            0xA0, 0x13, 0x01, 0x8B, 0x17, 0x30, 0x00, // head
            0xE2, 0x80, 0x69, 0x15, 0x00, 0x00, 0x50, 0x1D, 0x63, 0xE2, 0xA0, 0x4F, //EPC
            0xD3, 0x26,
        ];

        raw_packet.extend(vec![
            0xA0, 0x13, 0x01, 0x8B, 0x17, 0x30, 0x00, // head
            0xE2, 0x80, 0x69, 0x15, 0x00, 0x00, 0x40, 0x1D, 0x63, 0xE2, 0xA4, 0x4F, //EPC
            0xD2, 0x33,
        ]);

        raw_packet.extend(vec![
            0xA0, 0x13, 0x01, 0x8B, 0x17, 0x30, 0x00, // head
            0xE2, 0x80, 0x69, 0x15, 0x00, 0x00, 0x50, 0x1D, 0x63, 0xE2, 0x9C, 0x4F, //EPC
            0xD4, 0x29,
        ]);

        raw_packet.extend(vec![
            0xA0, 0x13, 0x01, 0x8B, 0x17, 0x30, 0x00, // head
            0xE2, 0x80, 0x69, 0x15, 0x00, 0x00, 0x40, 0x1D, 0x63, 0xE3, 0x28, 0x4F, //EPC
            0xC4, 0xBC,
        ]);

        raw_packet.extend(vec![
            0xA0, 0x0A, 0x01, 0x8B, // parte iniziale struttura pacchetto
            0x07, // ant ID
            0x00, 0x5A, // ReadRate
            0x00, 0x00, 0x00, 0x04, // Total read
            0x65, //Checksum
        ]);

        let result = parse_tag_response(raw_packet).unwrap();

        assert_eq!(result.1.antenna_id, 7);
        assert_eq!(result.1.read_rate, 90);
        assert_eq!(result.1.total_read, 4);
        assert_eq!(result.0.len(), 4);
        assert_eq!(result.0[0].epc, "E28069150000501D63E2A04F".to_string());
        assert_eq!(result.0[1].epc, "E28069150000401D63E2A44F".to_string());
        assert_eq!(result.0[2].epc, "E28069150000501D63E29C4F".to_string());
        assert_eq!(result.0[3].epc, "E28069150000401D63E3284F".to_string());
    }
}
