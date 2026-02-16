use super::*;
use crate::frame::command::{Command, CommandResult, PhaseStatus, RfLinkProfile, SerializableCommand, Session, Target};
use crate::tag::Tag;
use crate::tag_iterator::TagIterator;
use async_trait::async_trait;
use log::{debug, info, warn};
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

#[async_trait]
pub trait AsyncIO {
    type Socket: AsyncRead + AsyncWrite + Unpin + Send;

    async fn write(&mut self, data: &[u8]) -> io::Result<usize>;
    async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>;
    async fn write_all(&mut self, data: &[u8]) -> io::Result<()>;

    async fn send_frame(&mut self, frame: &[u8]) -> io::Result<()>;
    async fn read_response(&mut self) -> io::Result<Vec<u8>>;
    async fn send_command(&mut self, cmd: &Command) -> Result<(), ConnectorError>;
    async fn send_and_read_command(
        &mut self,
        cmd: Command,
    ) -> Result<CommandResult, ConnectorError>;
    ///
    /// Legge il comando di risposta, ma passiamo il comando inviato a cui dobbiamo ricevere risposta
    /// in modo che possiamo poi capire come parsare il dato
    async fn read_command(
        &mut self,
        sent_command: &Command,
    ) -> Result<CommandResult, ConnectorError>;
    async fn setup_reader(&mut self) -> Result<(), ConnectorError>;
    async fn set_frequency_if_not(
        &mut self,
        p0: Spectrum,
        p1: f64,
        p2: f64,
    ) -> Result<(), ConnectorError>;
    async fn set_output_power_if_not(&mut self, p0: Vec<u8>) -> Result<(), ConnectorError>;
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
    async fn build_fast_switching_antenna_cfg(
        &mut self,
        default_stay: u8,
    ) -> Result<Vec<(u8, u8)>, ConnectorError>;
    ///
    /// Return VSWR for every antenna
    ///
    async fn get_statistic_to_all_antennas(&mut self) -> Result<Vec<(u8, f64)>, ConnectorError>;
    async fn set_ant_connection_detector_if_not(&mut self, p0: u8) -> Result<(), ConnectorError>;
    async fn set_rf_link_profile_if_not(&mut self, p0: RfLinkProfile)
    -> Result<(), ConnectorError>;
    ///
    /// Read with 1 repeat on the working antenna
    async fn make_a_read_single_antenna(&mut self) -> Result<Vec<Tag>, ConnectorError>;
    //
    // Read with 1 repeat on the working antenna
    // antenna_cfg: a vector of tuple antenna_id e stay
    // async fn new_fast_switching_antenna_iterator(
    //      &mut self,
    //      antenna_cfg: Vec<(u8, u8)>,
    //  ) -> Result<TagIterator<'_, Self::Socket>, ConnectorError>;
}

#[async_trait]
impl<S> AsyncIO for Connector<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    type Socket = S;

    async fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.socket.write(data).await
    }

    async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.socket.read(buf).await
    }

    async fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
        self.socket.write_all(data).await
    }

    async fn send_frame(&mut self, frame: &[u8]) -> io::Result<()> {
        self.socket.write_all(frame).await?;
        self.socket.flush().await
    }

    // TODO codice parzialmente duplicato in sync e async
    async fn read_response(&mut self) -> io::Result<Vec<u8>> {
        let mut buffer = Vec::new();
        let mut temp = [0u8; 1024];
        let mut start = Instant::now();

        loop {
            match self.socket.read(&mut temp).await {
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

    async fn send_command(&mut self, cmd: &Command) -> Result<(), ConnectorError> {
        self.send_frame(&*command_to_frame_bytes(cmd)).await?;
        Ok(())
    }

    async fn send_and_read_command(
        &mut self,
        cmd: Command,
    ) -> Result<CommandResult, ConnectorError> {
        self.send_command(&cmd).await?;
        match core_send_and_read_command(self.read_command(&cmd).await) {
            Ok(ReadAction::Ok(result)) => Ok(result),
            Ok(ReadAction::Repeat) => self.send_and_read_command(cmd).await,
            Err(e) => Err(e),
            Ok(_) => unreachable!(),
        }
    }

    async fn read_command(
        &mut self,
        sent_command: &Command,
    ) -> Result<CommandResult, ConnectorError> {
        let response = timed_debug!("Response time:", self.read_response().await?);
        debug_print_vec("RX", &response);
        Ok(Command::from_byte(response, sent_command)?)
    }

    async fn setup_reader(&mut self) -> Result<(), ConnectorError> {
        info!("\n\n== Controllo antenna detection:");
        self.set_ant_connection_detector_if_not(0x03).await?; // TODO configurable, ma terrei attivo in modo che il fast switching rilevi errori di connessione

        info!("\n\n== Controllo frequenza:");
        self.set_frequency_if_not(
            self.working_freq_setup.0.clone(),
            self.working_freq_setup.1,
            self.working_freq_setup.2,
        )
        .await?;

        info!("\n\n== Controllo potenza:");
        self.set_output_power_if_not(self.output_power.clone())
            .await?;

        info!("\n\n== Controllo Rf Link Profile:");
        self.set_rf_link_profile_if_not(RfLinkProfile::Tari25usMiller4KHz250)
            .await?; //TODO configurable

        Ok(())
    }

    async fn set_frequency_if_not(
        &mut self,
        p0: Spectrum,
        p1: f64,
        p2: f64,
    ) -> Result<(), ConnectorError> {
        let response = self
            .send_and_read_command(Command::GetFrequencyRegion)
            .await?;

        match core_set_frequency_if_not(response, p0, p1, p2) {
            Ok(ReadAction::ExecuteCommand(command)) => {
                match self.send_and_read_command(command).await {
                    Ok(_) => Ok(()),
                    Err(e) => Err(e),
                }
            }
            Ok(ReadAction::Ok(_result)) => Ok(()),
            Err(e) => Err(e),
            Ok(_) => unreachable!(),
        }
    }

    async fn set_output_power_if_not(&mut self, p0: Vec<u8>) -> Result<(), ConnectorError> {
        let response = self.send_and_read_command(Command::GetOutputPower).await?;

        if let CommandResult::GetOutputPower(Ok(setted_values)) = response {
            if setted_values != p0 {
                debug!("NEED CHANGE OUTPUT POWER: {:?}", p0);
                self.send_and_read_command(Command::SetOutputPower(p0.clone()))
                    .await?;
            }
            Ok(())
        } else {
            Err(ConnectorError::FailedSetting(format!(
                "Failed to check Output Power for new settings {:?}",
                p0
            )))
        }
    }

    async fn build_fast_switching_antenna_cfg(
        &mut self,
        default_stay: u8,
    ) -> Result<Vec<(u8, u8)>, ConnectorError> {
        let antennas = self.get_statistic_to_all_antennas().await?;
        Ok(core_build_fast_switching_antennas(antennas, default_stay))
    }

    async fn get_statistic_to_all_antennas(&mut self) -> Result<Vec<(u8, f64)>, ConnectorError> {
        let mut antennas: Vec<(u8, f64)> = vec![];

        for antenna_id in 0..self.total_number_of_antennas {
            self.send_and_read_command(Command::SetWorkAntenna(antenna_id))
                .await?;

            let response = self
                .send_and_read_command(Command::GetRfPortReturnLoss(self.reference_frequency()))
                .await?;
            core_map_get_rf_port_return_loss(&mut antennas, antenna_id, response);
        }

        Ok(antennas)
    }

    async fn set_ant_connection_detector_if_not(&mut self, p0: u8) -> Result<(), ConnectorError> {
        let response = self.send_and_read_command(Command::GetAntConnectionDetector).await?;

        if let CommandResult::GetAntConnectionDetector(Ok(setted_values)) = response {
            if setted_values != p0 {
                debug!("NEED CHANGE ConnectionDetector value: {:?}", p0);
                self.send_and_read_command(Command::SetAntConnectionDetector(p0.clone())).await?;
            }
            Ok(())
        } else {
            Err(ConnectorError::FailedSetting(format!(
                "Failed to set Ant connection Error to desired settings {:?}",
                p0
            )))
        }
    }

    async fn set_rf_link_profile_if_not(
        &mut self,
        p0: RfLinkProfile,
    ) -> Result<(), ConnectorError> {
        let response = self.send_and_read_command(Command::GetRfLinkProfile).await?;
        if let CommandResult::GetRfLinkProfile(Ok(setted_values)) = response {
            if setted_values != p0 {
                debug!("NEED CHANGE RfLinkProfile to value: {:?}", p0);
                self.send_and_read_command(Command::SetRfLinkProfile(p0.clone())).await?;
            }
            Ok(())
        } else {
            Err(ConnectorError::FailedSetting(format!(
                "Failed to set RfLinkProfile to desired settings {:?}",
                p0
            )))
        }
    }

    async fn make_a_read_single_antenna(&mut self) -> Result<Vec<Tag>, ConnectorError> {
        let response = self.send_and_read_command(Command::CustomizeSessionTargetInventory(
            Session::S1,
            Target::A,
            PhaseStatus::Off,
            1,
        )).await?;
        debug!("Risposta ricevuta: {response}\n");

        if let CommandResult::ResponsePackets(Ok(setted_values)) = response {
            debug!("{:?}", setted_values);
            Ok(setted_values.0)
        } else {
            Err(ConnectorError::TagReadError(format!("Failed to read Tags")))
        }
    }
}
