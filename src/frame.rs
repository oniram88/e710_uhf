use std::fmt::{Display, Formatter};

const FRAME_HEADER: u8 = 0xA0;
const RS485_ADDRESS: u8 = 0x01;

#[derive(Debug)]
pub enum FrameError {
    InvalidCommand(String),
}

impl Display for FrameError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            FrameError::InvalidCommand(msg) => write!(f, "Invalid command: {}", msg),
        }
    }
}

impl std::error::Error for FrameError {}

pub enum Command {
    GetFirmwareVersion,
    GetTemperature,
}

impl Display for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Command::GetFirmwareVersion => write!(f, "Firmware Version"),
            Command::GetTemperature => write!(f, "Temperature"),
        }
    }
}

/// Trait for serializable commands
pub(crate) trait SerializableCommand {
    /// Returns a tuple of bytes (command, parameters)
    /// Parameters may be empty if not present
    fn to_bytes(&self) -> Vec<u8>;
    fn from_byte(raw: Vec<u8>) -> Result<Self, FrameError>
    where
        Self: Sized;
}

impl SerializableCommand for Command {
    ///
    /// Genera i bytes che identificano comando e dati nel caso di un comando con dati
    fn to_bytes(&self) -> Vec<u8> {
        match self {
            Command::GetFirmwareVersion => vec![0x72],
            Command::GetTemperature => vec![0x7B],
        }
    }

    fn from_byte(raw: Vec<u8>) -> Result<Self, FrameError> {
        match raw[0] {
            0x72 => {
                Ok(Command::GetFirmwareVersion)
            }
            0x7B => Ok(Command::GetTemperature),
            _ => Err(FrameError::InvalidCommand(format!(
                "Invalid command code: {}",
                raw[0]
            ))),
        }
    }
}

pub struct Frame {
    payload: Vec<u8>,
}

impl Frame {
    pub fn new(payload: &Command) -> Self {
        Frame { payload: payload.to_bytes() }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v = Vec::new();
        v.push(FRAME_HEADER); // HEADER
        v.push((self.payload.len()  + 2) as u8) ; // payload + un byte per address e un byte per checksum
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
        let cmd = Command::from_byte(vec![0x72]).unwrap();

        assert!(matches!(cmd, Command::GetFirmwareVersion));

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
}
