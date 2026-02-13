use crate::connector::{Connector, ConnectorError, ReadAction, TIMEOUT_WAITING_PACKET, command_to_frame_bytes, core_send_and_read_command, debug_print_vec, core_set_frequency_if_not};
use crate::frame::FrameError;
use crate::frame::command::{
    Command, CommandResult, PhaseStatus, RfLinkProfile, SerializableCommand, Session, Target,
};
use crate::frequency_references::Spectrum;
use crate::tag::Tag;
use crate::{tag_iterator, timed_debug};
use crate::tag_iterator::TagIterator;
use log::{debug, error, info, warn};
use std::io;
use std::io::{Read, Write};
use std::time::{Duration, Instant};

/// Trait per operazioni sincrone
pub trait SyncIO {
    type Socket: Read + Write;

    fn write(&mut self, data: &[u8]) -> io::Result<usize>;
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>;
    fn write_all(&mut self, data: &[u8]) -> io::Result<()>;
    fn send_frame(&mut self, frame: &[u8]) -> io::Result<()>;
    fn read_response(&mut self) -> io::Result<Vec<u8>>;
    fn send_command(&mut self, cmd: &Command) -> Result<(), ConnectorError>;
    fn send_and_read_command(&mut self, cmd: Command) -> Result<CommandResult, ConnectorError>;
    ///
    /// Legge il comando di risposta, ma passiamo il comando inviato a cui dobbiamo ricevere risposta
    /// in modo che possiamo poi capire come parsare il dato
    fn read_command(&mut self, sent_command: &Command) -> Result<CommandResult, ConnectorError>;
    fn setup_reader(&mut self) -> Result<(), ConnectorError>;
    fn set_frequency_if_not(
        &mut self,
        p0: Spectrum,
        p1: f64,
        p2: f64,
    ) -> Result<(), ConnectorError>;
    fn set_output_power_if_not(&mut self, p0: Vec<u8>) -> Result<(), ConnectorError>;
    ///   Builds a configuration for fast switching between antennas based on VSWR (Voltage Standing Wave Ratio).
    ///
    ///   The method filters out antennas with a VSWR value equal to or higher than 2.0 and assigns a
    ///   default "stay time" for the remaining antennas. The configuration is returned as a vector of
    ///   tuples containing the antenna ID and the default stay time.
    ///
    ///   # Parameters
    ///
    ///   * `default_stay` - A `u8` value that represents the default duration to stay on each antenna in the returned configuration.
    ///
    ///   # Returns
    ///
    ///   Returns a `Result`:
    ///
    ///   * `Ok(Vec<(u8, u8)>)` - A vector of tuples. Each tuple contains:
    ///       - `u8`: The antenna ID.
    ///       - `u8`: The default stay time.
    ///   * `Err(ConnectorError)` - An error occurs if retrieving statistics for the antennas fails.
    ///
    fn build_fast_switching_antenna_cfg(
        &mut self,
        default_stay: u8,
    ) -> Result<Vec<(u8, u8)>, ConnectorError>;
    ///
    /// Return VSWR for every antenna
    ///
    fn get_statistic_to_all_antennas(&mut self) -> Result<Vec<(u8, f64)>, ConnectorError>;
    fn set_ant_connection_detector_if_not(&mut self, p0: u8) -> Result<(), ConnectorError>;
    fn set_rf_link_profile_if_not(&mut self, p0: RfLinkProfile) -> Result<(), ConnectorError>;
    ///
    /// Read with 1 repeat on the working antenna
    fn make_a_read_single_antenna(&mut self) -> Result<Vec<Tag>, ConnectorError>;
    ///
    /// Read with 1 repeat on the working antenna
    /// antenna_cfg: a vector of tuple antenna_id e stay
    fn new_fast_switching_antenna_iterator(
        &mut self,
        antenna_cfg: Vec<(u8, u8)>,
    ) -> Result<TagIterator<'_, Self::Socket>, ConnectorError>;
}
/// Implementazione sincrona per qualsiasi socket che implementa Read + Write
impl<S> SyncIO for Connector<S>
where
    S: Read + Write,
{
    type Socket = S;

    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.socket.write(data)
    }

    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.socket.read(buf)
    }

    fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
        self.socket.write_all(data)
    }

    fn send_frame(&mut self, frame: &[u8]) -> io::Result<()> {
        self.socket.write_all(frame)?;
        self.socket.flush()
    }

    // TODO codice parzialmente duplicato in sync e async
    fn read_response(&mut self) -> io::Result<Vec<u8>> {
        let mut buffer = Vec::new();
        let mut temp = [0u8; 1024];
        let mut start = Instant::now();

        loop {
            match self.socket.read(&mut temp) {
                Ok(n) if n > 0 => {
                    buffer.extend_from_slice(&temp[..n]);
                    // resetta il timer se arrivano dati
                    start = Instant::now();
                }
                Ok(_) => {
                    if start.elapsed() > Duration::from_millis(TIMEOUT_WAITING_PACKET) {
                        debug!("Timeout waiting for response internal read");
                        break;
                    }
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                    // Questo timeout è definito tramite le impostazioni della seriale, la quale attende
                    // X tempo prima di emettere un timeout se non riceve alcun pacchetto
                    debug!("Error Timeout waiting for response");
                    break;
                }
                Err(e) => return Err(e),
            }
        }

        Ok(buffer)
    }

    fn send_command(&mut self, cmd: &Command) -> Result<(), ConnectorError> {
        self.send_frame(&*command_to_frame_bytes(cmd))?;
        Ok(())
    }

    fn send_and_read_command(&mut self, cmd: Command) -> Result<CommandResult, ConnectorError> {
        self.send_command(&cmd)?;
        match core_send_and_read_command(self.read_command(&cmd)) {
            Ok(ReadAction::Ok(result)) => Ok(result),
            Ok(ReadAction::Repeat) => self.send_and_read_command(cmd),
            Err(e) => Err(e),
            Ok(_) => unreachable!()
        }
    }

    ///
    /// Legge il comando di risposta, ma passiamo il comando inviato a cui dobbiamo ricevere risposta
    /// in modo che possiamo poi capire come parsare il dato
    fn read_command(&mut self, sent_command: &Command) -> Result<CommandResult, ConnectorError> {
        let response =  timed_debug!("Response time:",self.read_response()?);
        debug_print_vec("RX", &response);
        Ok(Command::from_byte(response, sent_command)?)
    }

    fn setup_reader(&mut self) -> Result<(), ConnectorError> {
        info!("\n\n== Controllo antenna detection:");
        self.set_ant_connection_detector_if_not(0x03)?; // TODO configurable, ma terrei attivo in modo che il fast switching rilevi errori di connessione

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

    fn set_frequency_if_not(
        &mut self,
        p0: Spectrum,
        p1: f64,
        p2: f64,
    ) -> Result<(), ConnectorError> {
        let response = self.send_and_read_command(Command::GetFrequencyRegion)?;

        match core_set_frequency_if_not(response, p0, p1, p2) {
            Ok(ReadAction::ExecuteCommand(command)) =>
                match self.send_and_read_command(command){
                    Ok(_)=>Ok(()),
                    Err(e)=>Err(e)
                }
            Ok(ReadAction::Ok(_result))=>Ok(()),
            Err(e) => Err(e),
            Ok(_) => unreachable!()
        }

    }

    fn set_output_power_if_not(&mut self, p0: Vec<u8>) -> Result<(), ConnectorError> {
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

    ///   Builds a configuration for fast switching between antennas based on VSWR (Voltage Standing Wave Ratio).
    ///
    ///   The method filters out antennas with a VSWR value equal to or higher than 2.0 and assigns a
    ///   default "stay time" for the remaining antennas. The configuration is returned as a vector of
    ///   tuples containing the antenna ID and the default stay time.
    ///
    ///   # Parameters
    ///
    ///   * `default_stay` - A `u8` value that represents the default duration to stay on each antenna in the returned configuration.
    ///
    ///   # Returns
    ///
    ///   Returns a `Result`:
    ///
    ///   * `Ok(Vec<(u8, u8)>)` - A vector of tuples. Each tuple contains:
    ///       - `u8`: The antenna ID.
    ///       - `u8`: The default stay time.
    ///   * `Err(ConnectorError)` - An error occurs if retrieving statistics for the antennas fails.
    ///
    fn build_fast_switching_antenna_cfg(
        &mut self,
        default_stay: u8,
    ) -> Result<Vec<(u8, u8)>, ConnectorError> {
        let mut out = vec![];
        let antennas = self.get_statistic_to_all_antennas()?;

        for (id_antenna, vswr) in antennas.iter() {
            // threshold per eliminare le antenne con vswr troppo alto
            if *vswr < 2.0 {
                out.push((*id_antenna, default_stay));
            }
        }

        Ok(out)
    }

    ///
    /// Return VSWR for every antenna
    ///
    fn get_statistic_to_all_antennas(&mut self) -> Result<Vec<(u8, f64)>, ConnectorError> {
        let mut antennas: Vec<(u8, f64)> = vec![];

        for antenna_id in 0..self.total_number_of_antennas {
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

    fn set_ant_connection_detector_if_not(&mut self, p0: u8) -> Result<(), ConnectorError> {
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

    fn set_rf_link_profile_if_not(&mut self, p0: RfLinkProfile) -> Result<(), ConnectorError> {
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
    fn make_a_read_single_antenna(&mut self) -> Result<Vec<Tag>, ConnectorError> {
        let response = self.send_and_read_command(Command::CustomizeSessionTargetInventory(
            Session::S1,
            Target::A,
            PhaseStatus::Off,
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
    /// antenna_cfg: a vector of tuple antenna_id e stay
    fn new_fast_switching_antenna_iterator(
        &mut self,
        antenna_cfg: Vec<(u8, u8)>,
    ) -> Result<TagIterator<'_, S>, ConnectorError> {
        let cmd = Command::FastSwitchAntInventory(
            antenna_cfg,
            0,
            Session::S1,
            Target::A,
            PhaseStatus::Off,
            1,
        );

        let iter_tag = tag_iterator::tag_stream(self, cmd, std::time::Duration::from_secs(0));

        Ok(iter_tag)
    }
}
