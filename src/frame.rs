use crate::error_references::ErrorCode;
use crate::frequency_references::{Spectrum, get_frequency, get_param};
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
    AntennaNotConnected
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
        }
    }
}

impl std::error::Error for FrameError {}

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
    // SetAntConnectionDetector
    // GetAntConnectionDetector,
    // SetTemporaryOutputPower,
    // SetReaderIdentifier,
    // GetReaderIdentifier,
    // SetRfLinkProfile,
    // GetRfLinkProfile,
    /// [GetRfPortReturnLoss] il valore impostato è la frequenza di controllo, dovrebbe essere
    /// la frequenza centrale di lavoro. ES. EU 865–868 MHz -> frequenza di check 866 MHz
    GetRfPortReturnLoss(f64),
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
            Command::GetRfPortReturnLoss(reference_frequency) => {
                write!(
                    f,
                    "Rf Port Return Loss setted with reference frequency of: {reference_frequency}"
                )
            }
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
            Command::Reset => vec![0xA0],
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
            Command::GetRfPortReturnLoss(reference_frequency) => {
                vec![0x7E, get_param(*reference_frequency)]
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

        let length = raw[1] as usize;
        let raw_command = raw[3];
        let checksum = raw[length + 1];
        let data = (&raw[4..(4 + length - 3)]).to_vec();

        debug!(
            "CMD[{}] DATA[{:?}] CHECKSUM[{}]",
            raw_command, data, checksum
        );

        match raw_command {
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
            0x79 => match data[0] {
                0x01 if length == 6 => Ok(CommandResult::GetFrequencyRegion(parse_response!(
                    data,
                    |data: Vec<u8>| Ok((
                        Spectrum::FCC,
                        get_frequency(data[1]),
                        get_frequency(data[2])
                    ))
                ))),
                0x02 if length == 6 => Ok(CommandResult::GetFrequencyRegion(parse_response!(
                    data,
                    |data: Vec<u8>| Ok((
                        Spectrum::ETSI,
                        get_frequency(data[1]),
                        get_frequency(data[2])
                    ))
                ))),
                0x03 if length == 6 => Ok(CommandResult::GetFrequencyRegion(parse_response!(
                    data,
                    |data: Vec<u8>| Ok((
                        Spectrum::CHN,
                        get_frequency(data[1]),
                        get_frequency(data[2])
                    ))
                ))),
                0x04 if length == 9 => {
                    // todo!("Da completare la versione impostata dall'utente");
                    Ok(CommandResult::GetFrequencyRegion(Ok((
                        Spectrum::CUSTOM,
                        0.0,
                        0.0,
                    ))))
                }
                _ => Err(FrameError::ResponseNotExpected(raw.clone())),
            },
            0x7E => Ok(CommandResult::GetRfPortReturnLoss(parse_response!(
                data,
                (0x00, 0x19),
                |data: Vec<u8>| {
                    println!("RF Port Return Loss: {:?}", data);

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

        v.push(checksum(&v));
        v
    }
}

fn checksum(buff: &[u8]) -> u8 {
    let mut sum: u8 = 0;

    for &b in buff {
        sum = sum.wrapping_add(b);
    }

    (!sum).wrapping_add(1)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(checksum(&data), 0xEA);
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
        let raw_packet = vec![0xA0, 0x04, 0x01, 0x75, 0x00, 0xEA];
        let result = Command::from_byte(raw_packet).unwrap();

        if let CommandResult::GetWorkAntenna(Ok(pos)) = result {
            assert_eq!(pos, 1);
        } else {
            panic!("Expected GetWorkAntenna(Ok), got {:?}", result);
        }
    }
}
