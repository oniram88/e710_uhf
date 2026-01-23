use std::cmp::PartialEq;
use crate::frame::{Command, CommandResult, Frame, FrameError, SerializableCommand};
use log::debug;
use std::fmt;
use std::io::{self, Read, Write};
use crate::frequency_references::Spectrum;

pub struct Connector<P>
where
    P: Read + Write,
{
    port: P,
}

#[derive(Debug)]
pub enum ConnectorError {
    Io(io::Error),
    Timeout,
    FailedSetting(String),
    SerialRead(String),
    Frame(FrameError),
}

impl fmt::Display for ConnectorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectorError::Io(e) => write!(f, "IO error: {}", e),
            ConnectorError::Timeout => write!(f, "Timeout"),
            ConnectorError::SerialRead(msg) => write!(f, "Serial read error: {}", msg),
            ConnectorError::FailedSetting(msg) => write!(f, "Failed Setting: {}", msg),
            ConnectorError::Frame(err) => write!(f, "Frame error: {}", err),
        }
    }
}

impl From<io::Error> for ConnectorError {
    fn from(err: io::Error) -> Self {
        ConnectorError::Io(err)
    }
}

impl From<FrameError> for ConnectorError {
    fn from(err: FrameError) -> Self {
        ConnectorError::Frame(err)
    }
}

impl<P> Connector<P>
where
    P: Read + Write,
{
    pub fn new(p0: P) -> Self {
        Connector { port: p0 }
    }

    pub fn send_frame(&mut self, frame: &[u8]) -> io::Result<()> {
        self.port.write_all(frame)?;
        self.port.flush()
    }

    pub fn read_response(&mut self) -> io::Result<Vec<u8>> {
        let mut buffer = [0u8; 1024];
        let n = self.port.read(&mut buffer)?;
        Ok(buffer[..n].to_vec())
    }

    pub fn send_command(&mut self, cmd: Command) -> Result<(), ConnectorError> {
        let frame = Frame::new(&cmd);
        let bytes = frame.to_bytes();

        debug!("[TX] {:02X?} - [{cmd}]", bytes);
        println!("[TX] {:02X?} - [{cmd}]", bytes);
        self.send_frame(&bytes)?;
        Ok(())
    }

    pub fn read_command(&mut self) -> Result<CommandResult, ConnectorError> {
        let response = self.read_response()?;
        debug!("[RX] {:02X?}", response);
        println!(
            "[RX] [{}]",
            response
                .iter()
                .map(|b| format!("0x{:02X}", b))
                .collect::<Vec<_>>()
                .join(",")
        );
        Ok(Command::from_byte(response)?)
    }

    pub fn set_frequency_if_not(&mut self, p0: Spectrum, p1: f64, p2: f64) -> Result<(), ConnectorError> {

        self.send_command(Command::GetFrequencyRegion)?;
        let response = self.read_command()?;

        if let CommandResult::GetFrequencyRegion(Ok(region)) = response {
            if region.0 != p0 || region.1 != p1 || region.2 != p2 {
                debug!("NEED CHANGE FREQUENCY REGION: {} {} {}",p0, p1, p2);
                self.send_command(Command::SetDefaultFrequencyRegion(p0, p1, p2))?;
                self.read_command()?;
            }
        }

        Ok(())
    }

}
