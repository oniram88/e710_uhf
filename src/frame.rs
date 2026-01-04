use std::fmt::{Display, Formatter};

pub const FRAME_HEADER: u8 = 0xA0;

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

pub(crate) enum Command {
    GetFirmwareVersion,
}

impl Display for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Command::GetFirmwareVersion => write!(f, "Firmware Version"),
        }
    }
}

/// Trait for serializable commands
pub(crate) trait SerializableCommand {
    /// Returns a tuple of bytes (command, parameters)
    /// Parameters may be empty if not present
    fn to_bytes(&self) -> (Vec<u8>);
    fn from_byte(raw: Vec<u8>) -> Result<Self, FrameError>
    where
        Self: Sized;
}

impl SerializableCommand for Command {
    ///
    /// Genera i bytes che identificano comando e dati nel caso di un comando con dati
    fn to_bytes(&self) -> (Vec<u8>) {
        match self {
            Command::GetFirmwareVersion => (vec![0x72]),
        }
    }

    fn from_byte(raw: Vec<u8>) -> Result<Self, FrameError> {
        match (raw[0]) {
            (0x72) => {
                Ok(Command::GetFirmwareVersion)
            }
            _ => Err(FrameError::InvalidCommand(format!(
                "Invalid command code: {}",
                raw[0]
            ))),
        }
    }
}

pub(crate) struct Frame {
    payload: Vec<u8>,
}

impl Frame {
    pub(crate) fn new(payload: &Command) -> Self {
        Frame { payload: payload.to_bytes() }
    }

    pub(crate) fn to_bytes(&self) -> Vec<u8> {
        let mut v = Vec::new();
        v.push(FRAME_HEADER); // HEADER
        v.push((self.payload.len()  + 2) as u8) ; // payload + un byte per address e un byte per checksum
        v.push(0xFF); //pubblic address for RS-485

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

   
}
