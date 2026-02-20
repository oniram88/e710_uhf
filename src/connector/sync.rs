use crate::connector::{
    Connector, ConnectorError, command_to_frame_bytes, core_build_fast_switching_antennas,
    core_map_get_rf_port_return_loss, debug_print_vec,
};
use crate::frame::FrameError;
use crate::frame::command::{
    Command, CommandResult, PhaseStatus, RfLinkProfile, Session, Target, try_parsing_results,
};
use crate::frequency_references::Spectrum;
use crate::tag::Tag;
use crate::tag_iterator;
use crate::tag_iterator::TagIterator;
use log::{debug, error, info};
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
    fn read_response(&mut self, sent_command: &Command) -> io::Result<CommandResult>;
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
    fn read_response(&mut self, sent_command: &Command) -> io::Result<CommandResult> {
        let mut buffer = Vec::new();
        let mut temp = [0u8; 1024];
        let mut start = Instant::now();

        loop {
            //dobbiamo inserire un micro sleep per dare il tempo al chip sottostante di inviarci i dati
            std::thread::sleep(Duration::from_micros(1300));
            match self.socket.read(&mut temp) {
                Ok(n) if n > 0 => {
                    buffer.extend_from_slice(&temp[..n]);

                    if buffer.len() > 2
                        && let Some(o) = try_parsing_results(buffer.as_ref(), sent_command)
                    {
                        debug_print_vec("RX", &buffer);
                        return Ok(o);
                    }

                    // resetta il timer se arrivano dati
                    start = Instant::now();
                }
                Ok(_) => {
                    if start.elapsed() > self.timeout_waiting_packet {
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

        Err(io::ErrorKind::Other.into())
    }

    fn send_command(&mut self, cmd: &Command) -> Result<(), ConnectorError> {
        self.send_frame(&*command_to_frame_bytes(cmd))?;
        Ok(())
    }

    fn send_and_read_command(&mut self, cmd: Command) -> Result<CommandResult, ConnectorError> {
        self.send_command(&cmd)?;
        match self.read_command(&cmd) {
            Ok(result) => Ok(result),
            Err(ConnectorError::Frame(FrameError::InvalidPacketOrder(
                sent_command,
                raw_response,
            ))) => {
                // Facciamo un loop per il momento
                error!(
                    "InvalidPacketOrder {sent_command} - {:?} - Make Loop?? -",
                    raw_response
                );
                self.send_and_read_command(cmd)
            }
            Err(e) => Err(e),
        }
    }

    ///
    /// Legge il comando di risposta, ma passiamo il comando inviato a cui dobbiamo ricevere risposta
    /// in modo che possiamo poi capire come parsare il dato
    fn read_command(&mut self, sent_command: &Command) -> Result<CommandResult, ConnectorError> {
        timed_debug!(
            "Response time:",
            match self.read_response(sent_command) {
                Ok(res) => Ok(res),
                Err(e) => Err(ConnectorError::Io(e)),
            }
        )
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

    fn build_fast_switching_antenna_cfg(
        &mut self,
        default_stay: u8,
    ) -> Result<Vec<(u8, u8)>, ConnectorError> {
        let antennas = self.get_statistic_to_all_antennas()?;
        Ok(core_build_fast_switching_antennas(antennas, default_stay))
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

            core_map_get_rf_port_return_loss(&mut antennas, antenna_id, response);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::command::ReadResult;
    use crate::frequency_references::Spectrum;
    use std::io::{self, Read, Write};

    // Mock sincrono minimale per simulare sequenze di lettura
    struct MockPort {
        read_data: Vec<Result<Vec<u8>, io::Error>>,
        idx: usize,
    }

    impl MockPort {
        fn new(read_data: Vec<Result<Vec<u8>, io::Error>>) -> Self {
            Self { read_data, idx: 0 }
        }
    }

    impl Read for MockPort {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.idx >= self.read_data.len() {
                // Simula fine dati: Ok(0)
                return Ok(0);
            }
            match &self.read_data[self.idx] {
                Ok(chunk) => {
                    let n = chunk.len().min(buf.len());
                    buf[..n].copy_from_slice(&chunk[..n]);
                    self.idx += 1;
                    Ok(n)
                }
                Err(e) => {
                    let kind = e.kind();
                    self.idx += 1;
                    Err(io::Error::new(kind, "mock error"))
                }
            }
        }
    }

    impl Write for MockPort {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    // --- Test per read_response() sincrono ---

    #[test]
    fn test_sync_read_response_single_chunk_without_tags() {
        let mock = MockPort::new(vec![Ok(vec![
            0xA0, 0x0A, 0x01, 0x8A, //header
            0x00, 0x00, 0x00, // total
            0x00, 0x00, 0x00, 0x79, // duration
            0x52,
        ])]);
        let mut conn = Connector::new(mock, 1, vec![30], (Spectrum::CHN, 920.125, 924.875), None);

        let rs = conn
            .read_response(&Command::FastSwitchAntInventory(
                vec![(1, 1)],
                0,
                Session::S0,
                Target::A,
                PhaseStatus::Off, // Phase disattivata
                0,
            ))
            .unwrap();
        assert_eq!(
            rs,
            CommandResult::ResponsePackets(Ok((
                vec![],
                ReadResult {
                    antenna_id: 0,
                    read_rate: 0,
                    total_read: 0
                }
            )))
        );
    }

    #[test]
    fn test_sync_read_response_single_chunk_with_tags() {
        let raw_packet = vec![
            0xA0, 0x15, 0x01, 0x8A, 0x00, 0x34, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x96, 0x41, 0x47, 0x01, 0xD8, 0x95, 0xA0, 0x15, 0x01, 0x8A, 0x00,
            0x30, 0x00, 0x30, 0x39, 0x5D, 0xFA, 0x82, 0xE3, 0x79, 0x00, 0x00, 0x30, 0x57, 0xA4,
            0x4E, 0x00, 0x87, 0xF2, 0xA0, 0x15, 0x01, 0x8A, 0x00, 0x34, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x96, 0x31, 0x48, 0x0E, 0x35, 0x3A, 0xA0,
            0x0A, 0x01, 0x8A, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x28, 0xA0,
        ];

        let mock = MockPort::new(vec![Ok(raw_packet)]);
        let mut conn = Connector::new(mock, 1, vec![30], (Spectrum::CHN, 920.125, 924.875), None);

        let rs = conn
            .read_response(&Command::FastSwitchAntInventory(
                vec![(1, 1)],
                0,
                Session::S0,
                Target::A,
                PhaseStatus::Off, // Phase disattivata
                0,
            ))
            .unwrap();

        if let CommandResult::ResponsePackets(Ok((tags, read_result))) = rs {
            assert_eq!(
                read_result,
                ReadResult {
                    antenna_id: 0,
                    read_rate: 13,
                    total_read: 3
                }
            );

            assert_eq!(tags.len(), 3);

            assert_eq!(tags[0].epc, "000000000000000000009631480E");
            assert_eq!(tags[1].epc, "30395DFA82E37900003057A44E00");
            assert_eq!(tags[2].epc, "0000000000000000000096414701");
        }
    }
}
