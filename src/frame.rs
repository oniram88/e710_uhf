use crate::frequency_references::{Spectrum, get_frequency, get_param};
use log::debug;
use std::fmt::{Display, Formatter};
use crate::error_references::ErrorCode;

const FRAME_HEADER: u8 = 0xA0;
const RS485_ADDRESS: u8 = 0x01;

#[derive(Debug)]
pub enum FrameError {
    InvalidCommand(String),
    ResponseNotExpected(Vec<u8>),
    InvalidPacket(Vec<u8>),
    FailedResponse(ErrorCode, Vec<u8>),
}

impl Display for FrameError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            FrameError::InvalidCommand(msg) => write!(f, "Invalid command: {}", msg),
            FrameError::ResponseNotExpected(response) => {
                write!(f, "Response not expected [RX] {:02X?}", response)
            }
            FrameError::InvalidPacket(packet) => write!(f, "Invalid packet [RX] {:02X?}", packet),
            FrameError::FailedResponse(code, packet) => write!(f, "Failed response with code {:?} and DATA {:02X?}", code, packet),
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
    // SetWorkAntenna,
    GetWorkAntenna,
    // SetOutputPower,
    // GetOutputPower,
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
    // GetRfPortReturnLoss
}

#[derive(Debug)]
pub enum CommandResult {
    GetFirmwareVersion(Result<(u8, u8), FrameError>),
    GetWorkAntenna(Result<u8, FrameError>), //posizione antenna
    GetReaderTemperature(Result<f64, FrameError>),
    SetDefaultFrequencyRegion(Result<(), FrameError>),
    GetFrequencyRegion(Result<String, FrameError>),
}

impl Display for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Command::Reset => write!(f, "Reset"),
            Command::GetWorkAntenna => write!(f, "Work Antenna"),
            Command::GetFirmwareVersion => write!(f, "Firmware Version"),
            Command::GetReaderTemperature => write!(f, "Temperature"),
            Command::SetDefaultFrequencyRegion(spectrum, min, max) => write!(f, "Set {spectrum} Frequency Region [{min} -> {max}]"),
            Command::GetFrequencyRegion => write!(f, "Frequency Region"),
        }
    }
}

impl Display for CommandResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandResult::GetFirmwareVersion(Ok((major, minor))) => {
                write!(f, "Firmware Version [{major}.{minor}]")
            }
            CommandResult::GetFirmwareVersion(Err(err)) => write!(f, "Failed to get Firmware Version: {}", err),
            CommandResult::GetWorkAntenna(Ok(pos)) => write!(f, "Work Antenna [{pos}]"),
            CommandResult::GetWorkAntenna(Err(err)) => write!(f, "Failed to get Work Antenna: {}", err),
            CommandResult::GetReaderTemperature(Ok(tmp)) => write!(f, "Temperature [{tmp} °C]"),
            CommandResult::GetReaderTemperature(Err(err)) => write!(f, "Failed to get Temperature: {}", err),
            CommandResult::SetDefaultFrequencyRegion(Ok(())) => write!(f, "Frequency Region set successfully"),
            CommandResult::SetDefaultFrequencyRegion(Err(err)) => write!(f, "Failed to set Frequency Region: {}", err),
            CommandResult::GetFrequencyRegion(Ok(region)) => write!(f, "Frequency Region [{region}]"),
            CommandResult::GetFrequencyRegion(Err(err)) => write!(f, "Failed to get Frequency Region: {}", err),
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

impl SerializableCommand for Command {
    ///
    /// Genera i bytes che identificano comando e dati nel caso di un comando con dati
    fn to_bytes(&self) -> Vec<u8> {
        match self {
            Command::Reset => vec![0xA0],
            Command::GetWorkAntenna => vec![0x75],
            Command::GetFirmwareVersion => vec![0x72],
            Command::GetReaderTemperature => vec![0x7B],
            Command::SetDefaultFrequencyRegion(spectrum, min, max) => {
                let mut v = vec![0x78];
                v.push(spectrum.clone() as u8);
                v.push(get_param(*min));
                v.push(get_param(*max));
                v
            }
            Command::GetFrequencyRegion => vec![0x79],
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
            0x72 => Ok(CommandResult::GetFirmwareVersion(Ok((data[0], data[1])))),
            0x75 => Ok(CommandResult::GetWorkAntenna(Ok(data[0] + 1))),
            0x7B => {
                let sign: f64 = if data[0] == 0x00 { -1.0 } else { 1.0 };

                Ok(CommandResult::GetReaderTemperature(Ok(data[1] as f64 * sign)))
            }
            0x78 => {
                Ok(CommandResult::SetDefaultFrequencyRegion(build_response_from_code(data)))
            },
            0x79 => {
                match data[0] {
                    0x01 if length == 6 => Ok(CommandResult::GetFrequencyRegion(Ok(format!(
                        "FCC {}->{}",
                        get_frequency(data[1]),
                        get_frequency(data[2])
                    )))),
                    0x02 if length == 6 => Ok(CommandResult::GetFrequencyRegion(Ok(format!(
                        "ETSI {}->{}",
                        get_frequency(data[1]),
                        get_frequency(data[2])
                    )))),
                    0x03 if length == 6 => Ok(CommandResult::GetFrequencyRegion(Ok(format!(
                        "CHN {}->{}",
                        get_frequency(data[1]),
                        get_frequency(data[2])
                    )))),
                    0x04 if length == 9 => {
                        // todo!("Da completare la versione impostata dall'utente");
                        Ok(CommandResult::GetFrequencyRegion(Ok("CUSTOM".to_string())))
                    }
                    _ => Err(FrameError::ResponseNotExpected(raw.clone())),
                }
            }
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

fn build_response_from_code(data: Vec<u8>) -> Result<(), FrameError> {
    match ErrorCode::from_hex(data[0]) {
        ErrorCode::CommandSuccess => Ok(()),
        response_error => Err(FrameError::FailedResponse(response_error,data)),
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

        assert!(matches!(cmd, CommandResult::GetFirmwareVersion(Ok(ref v)) if *v == expected_version));

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
            assert_eq!(region, "FCC 902->928");
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
