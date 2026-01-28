use crate::error_references::ErrorCode;
use crate::frame::Command;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum FrameError {
    InvalidCommand(String),
    ResponseNotExpected(Vec<u8>),
    InvalidPacket(Vec<u8>),
    FailedResponse(ErrorCode, Vec<u8>),
    AntennaNotConnected,
    TagParsingError(Vec<u8>),
    InvalidChecksum,
    InvalidSentCommand(Command),
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
            FrameError::InvalidSentCommand(command) => write!(
                f,
                "Abbiamo ricevuto come comando trasmesso un comando non previsto [{:#?}]",
                command
            ),
        }
    }
}

impl std::error::Error for FrameError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_error_display() {
        let err = FrameError::InvalidCommand("test".to_string());
        assert_eq!(format!("{}", err), "Invalid command: test");
    }
}
