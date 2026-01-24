use crate::frame::{Command, CommandResult, Frame, FrameError, RfLinkProfile, SerializableCommand};
use crate::frequency_references::Spectrum;
use log::debug;
use std::cmp::PartialEq;
use std::fmt;
use std::io::{self, Read, Write};

pub struct Connector<P>
where
    P: Read + Write,
{
    port: P,
    number_of_antennas: u8,
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
    pub fn new(p0: P, number_of_antennas: u8) -> Self {
        Connector {
            port: p0,
            number_of_antennas,
        }
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

    pub fn set_frequency_if_not(
        &mut self,
        p0: Spectrum,
        p1: f64,
        p2: f64,
    ) -> Result<(), ConnectorError> {
        self.send_command(Command::GetFrequencyRegion)?;
        let response = self.read_command()?;

        if let CommandResult::GetFrequencyRegion(Ok(region)) = response {
            if region.0 != p0 || region.1 != p1 || region.2 != p2 {
                debug!("NEED CHANGE FREQUENCY REGION: {} {} {}", p0, p1, p2);
                self.send_command(Command::SetDefaultFrequencyRegion(p0, p1, p2))?;
                self.read_command()?;
            }
            Ok(())
        } else {
            Err(ConnectorError::FailedSetting(format!(
                "Failed to check Frequency Region for new settings {:?} {:?} {:?}",
                p0, p1, p2
            )))
        }
    }

    pub fn set_output_power_if_not(&mut self, p0: Vec<u8>) -> Result<(), ConnectorError> {
        self.send_command(Command::GetOutputPower)?;
        let response = self.read_command()?;

        if let CommandResult::GetOutputPower(Ok(setted_values)) = response {
            if setted_values != p0 {
                debug!("NEED CHANGE OUTPUT POWER: {:?}", p0);
                self.send_command(Command::SetOutputPower(p0.clone()))?;
                self.read_command()?;
            }
            Ok(())
        } else {
            Err(ConnectorError::FailedSetting(format!(
                "Failed to check Output Power for new settings {:?}",
                p0
            )))
        }
    }

    pub fn check_all_antennas_rf_port_return_loss(
        &mut self,
        reference_frequency: f64,
    ) -> Result<(), ConnectorError> {
        for antenna_id in 0..self.number_of_antennas {
            self.send_command(Command::SetWorkAntenna(antenna_id))?;
            self.read_command()?;

            self.send_command(Command::GetRfPortReturnLoss(reference_frequency))?;
            let response = self.read_command()?;

            if let CommandResult::GetRfPortReturnLoss(vswr_res) = response {
                match vswr_res {
                    Ok(vswr) => {
                        println!("Antenna {}: VSWR = {:.2}", antenna_id, vswr);
                    }
                    Err(e) => {
                        println!("Antenna {}: Error getting Return Loss: {}", antenna_id, e);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn set_ant_connection_detector_if_not(&mut self, p0: u8) -> Result<(), ConnectorError> {
        self.send_command(Command::GetAntConnectionDetector)?;
        let response = self.read_command()?;

        if let CommandResult::GetAntConnectionDetector(Ok(setted_values)) = response {
            if setted_values != p0 {
                debug!("NEED CHANGE ConnectionDetector value: {:?}", p0);
                self.send_command(Command::SetAntConnectionDetector(p0.clone()))?;
                self.read_command()?;
            }
            Ok(())
        } else {
            Err(ConnectorError::FailedSetting(format!(
                "Failed to set Ant connection Error to desired settings {:?}",
                p0
            )))
        }
    }

    pub fn set_rf_link_profile_if_not(&mut self, p0: RfLinkProfile) -> Result<(), ConnectorError> {
        self.send_command(Command::GetRfLinkProfile)?;
        let response = self.read_command()?;
        if let CommandResult::GetRfLinkProfile(Ok(setted_values)) = response {
            if setted_values != p0 {
                debug!("NEED CHANGE RfLinkProfile to value: {:?}", p0);
                self.send_command(Command::SetRfLinkProfile(p0.clone()))?;
                self.read_command()?;
            }
            Ok(())
        } else {
            Err(ConnectorError::FailedSetting(format!(
                "Failed to set RfLinkProfile to desired settings {:?}",
                p0
            )))
        }
    }
}
