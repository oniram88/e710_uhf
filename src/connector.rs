use crate::frame::{
    Command, CommandResult, Frame, FrameError, RfLinkProfile, SerializableCommand, Session, Target,
};
use crate::frequency_references::Spectrum;
use crate::tag::Tag;
use crate::tag_iterator;
use log::{debug, error, info, warn};
use std::cmp::PartialEq;
use std::fmt;
use std::io::{self, Read, Write};

pub struct Connector<P>
where
    P: Read + Write,
{
    port: P,
    number_of_antennas: u8,
    /// Potenza di lavoro da 0 a 33 db
    /// con un solo valore andremo ad impostare su tutte le antenne la medesima potenza
    /// con più valori ogni antenna avrà la sua potenza distinta
    output_power: Vec<u8>,
    working_freq_setup: (Spectrum, f64, f64),
}

#[derive(Debug)]
pub enum ConnectorError {
    Io(io::Error),
    Timeout,
    FailedSetting(String),
    SerialRead(String),
    Frame(FrameError),
    TagReadError(String),
}

impl fmt::Display for ConnectorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectorError::Io(e) => write!(f, "IO error: {}", e),
            ConnectorError::Timeout => write!(f, "Timeout"),
            ConnectorError::SerialRead(msg) => write!(f, "Serial read error: {}", msg),
            ConnectorError::FailedSetting(msg) => write!(f, "Failed Setting: {}", msg),
            ConnectorError::Frame(err) => write!(f, "Frame error: {}", err),
            ConnectorError::TagReadError(msg) => write!(f, "Tag Read Error: {}", msg),
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
    pub fn new(
        p0: P,
        number_of_antennas: u8,
        output_power: Vec<u8>,
        working_freq_setup: (Spectrum, f64, f64),
    ) -> Self {
        Connector {
            port: p0,
            number_of_antennas,
            working_freq_setup,
            output_power,
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

    pub fn send_command(&mut self, cmd: &Command) -> Result<(), ConnectorError> {
        let frame = Frame::new(&cmd);
        let bytes = frame.to_bytes();

        debug!("[TX] {:02X?} - [{cmd}]", bytes);
        self.send_frame(&bytes)?;
        Ok(())
    }

    pub fn send_and_read_command(&mut self, cmd: Command) -> Result<CommandResult, ConnectorError> {
        self.send_command(&cmd)?;
        self.read_command(&cmd)
    }

    ///
    /// Legge il comando di risposta, ma passiamo il comando inviato a cui dobbiamo ricevere risposta
    /// in modo che possiamo poi capire come parsare il dato
    pub fn read_command(
        &mut self,
        sent_command: &Command,
    ) -> Result<CommandResult, ConnectorError> {
        let response = self.read_response()?;
        debug!(
            "[RX] [{}]",
            response
                .iter()
                .map(|b| format!("0x{:02X}", b))
                .collect::<Vec<_>>()
                .join(",")
        );
        Ok(Command::from_byte(response, sent_command)?)
    }

    pub fn setup_reader(&mut self) -> Result<(), ConnectorError> {
        info!("\n\n== Controllo antenna detection:");
        self.set_ant_connection_detector_if_not(0x03).unwrap(); // TODO configurable

        info!("\n\n== Controllo frequenza:");
        self.set_frequency_if_not(
            self.working_freq_setup.0.clone(),
            self.working_freq_setup.1,
            self.working_freq_setup.2,
        )?;

        info!("\n\n== Controllo potenza:");
        self.set_output_power_if_not(self.output_power.clone())?;

        info!("\n\n== Controllo Rf Link Profile:");
        self.set_rf_link_profile_if_not(RfLinkProfile::Tari25usMiller4KHz250)?; //TODO configurable

        Ok(())
    }

    pub fn set_frequency_if_not(
        &mut self,
        p0: Spectrum,
        p1: f64,
        p2: f64,
    ) -> Result<(), ConnectorError> {
        let response = self.send_and_read_command(Command::GetFrequencyRegion)?;

        if let CommandResult::GetFrequencyRegion(Ok(region)) = response {
            if region.0 != p0 || region.1 != p1 || region.2 != p2 {
                debug!("NEED CHANGE FREQUENCY REGION: {} {} {}", p0, p1, p2);
                self.send_and_read_command(Command::SetDefaultFrequencyRegion(p0, p1, p2))?;
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
        let response = self.send_and_read_command(Command::GetOutputPower)?;

        if let CommandResult::GetOutputPower(Ok(setted_values)) = response {
            if setted_values != p0 {
                debug!("NEED CHANGE OUTPUT POWER: {:?}", p0);
                self.send_and_read_command(Command::SetOutputPower(p0.clone()))?;
            }
            Ok(())
        } else {
            Err(ConnectorError::FailedSetting(format!(
                "Failed to check Output Power for new settings {:?}",
                p0
            )))
        }
    }

    pub fn reference_frequency(&self) -> f64 {
        ((self.working_freq_setup.1 + self.working_freq_setup.2) / 2.0).trunc()
    }

    // pub fn check_all_antennas_rf_port_return_loss(
    //     &mut self,
    //     reference_frequency: f64,
    // ) -> Result<(), ConnectorError> {
    //     for antenna_id in 0..self.number_of_antennas {
    //         self.send_command(Command::SetWorkAntenna(antenna_id))?;
    //         self.read_command()?;
    //
    //         self.send_command(Command::GetRfPortReturnLoss(reference_frequency))?;
    //         let response = self.read_command()?;
    //
    //         if let CommandResult::GetRfPortReturnLoss(vswr_res) = response {
    //             match vswr_res {
    //                 Ok(vswr) => {
    //                     println!("Antenna {}: VSWR = {:.2}", antenna_id, vswr);
    //                 }
    //                 Err(e) => {
    //                     println!("Antenna {}: Error getting Return Loss: {}", antenna_id, e);
    //                 }
    //             }
    //         }
    //     }
    //     Ok(())
    // }

    ///
    /// Return VSWR for every antenna
    ///
    pub fn get_statistic_to_all_antennas(&mut self) -> Result<Vec<(u8, f64)>, ConnectorError> {
        let mut antennas: Vec<(u8, f64)> = vec![];

        for antenna_id in 0..self.number_of_antennas {
            self.send_and_read_command(Command::SetWorkAntenna(antenna_id))?;

            let response = self
                .send_and_read_command(Command::GetRfPortReturnLoss(self.reference_frequency()))?;

            if let CommandResult::GetRfPortReturnLoss(vswr_res) = response {
                match vswr_res {
                    Ok(vswr) => {
                        antennas.push((antenna_id, vswr));
                        info!("Antenna {}: VSWR = {:.2}", antenna_id, vswr);
                    }
                    Err(e) => {
                        warn!("Antenna {}: Error getting Return Loss: {}", antenna_id, e);
                    }
                }
            }
        }

        Ok(antennas)
    }

    pub fn set_ant_connection_detector_if_not(&mut self, p0: u8) -> Result<(), ConnectorError> {
        let response = self.send_and_read_command(Command::GetAntConnectionDetector)?;

        if let CommandResult::GetAntConnectionDetector(Ok(setted_values)) = response {
            if setted_values != p0 {
                debug!("NEED CHANGE ConnectionDetector value: {:?}", p0);
                self.send_and_read_command(Command::SetAntConnectionDetector(p0.clone()))?;
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
        let response = self.send_and_read_command(Command::GetRfLinkProfile)?;
        if let CommandResult::GetRfLinkProfile(Ok(setted_values)) = response {
            if setted_values != p0 {
                debug!("NEED CHANGE RfLinkProfile to value: {:?}", p0);
                self.send_and_read_command(Command::SetRfLinkProfile(p0.clone()))?;
            }
            Ok(())
        } else {
            Err(ConnectorError::FailedSetting(format!(
                "Failed to set RfLinkProfile to desired settings {:?}",
                p0
            )))
        }
    }

    ///
    /// Read with 1 repeat on the working antenna
    pub fn make_a_read_single_antenna(&mut self) -> Result<Vec<Tag>, ConnectorError> {
        let response = self.send_and_read_command(Command::CustomizeSessionTargetInventory(
            Session::S1,
            Target::A,
            0,
            1,
        ))?;
        debug!("Risposta ricevuta: {response}\n");

        if let CommandResult::ResponsePackets(Ok(setted_values)) = response {
            debug!("{:?}", setted_values);
            Ok(setted_values.0)
        } else {
            Err(ConnectorError::TagReadError(format!("Failed to read Tags")))
        }
    }

    ///
    /// Read with 1 repeat on the working antenna
    pub fn read_fast_switching_antenna_read(&mut self) -> Result<Vec<Tag>, ConnectorError> {
        let mut out = Vec::new();
        let cmd = Command::FastSwitchAntInventory(
            vec![(0, 1), (1, 1), (6, 1), (7, 1)],
            0,
            Session::S1,
            Target::A,
            0,
            1,
        );
        self.send_command(&cmd)?;

        let mut iter_tag = tag_iterator::tag_stream(self,&cmd, std::time::Duration::from_secs(0));

        while let Some(res) = iter_tag.next() {
            match res {
                Ok(tag) => {
                    out.push(tag);
                },
                Err(e) => error!("Error reading tags: {:?}", e),
            }
        }

        Ok(out)
    }
}
