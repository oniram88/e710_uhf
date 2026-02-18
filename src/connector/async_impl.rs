use super::*;
use crate::frame::command::{
    Command, CommandResult, PhaseStatus, RfLinkProfile, SerializableCommand, Session, Target,
};
use crate::tag::Tag;
use crate::tag_stream_async;
use async_trait::async_trait;
use bytes::BytesMut;
use futures_core::Stream;
use log::{debug, error, info};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::time::{Duration, timeout};

#[async_trait]
pub trait AsyncIO {
    type Socket: AsyncRead + AsyncWrite + Unpin + Send;

    async fn write(&mut self, data: &[u8]) -> io::Result<usize>;
    async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>;
    async fn write_all(&mut self, data: &[u8]) -> io::Result<()>;

    async fn send_frame(&mut self, frame: &[u8]) -> io::Result<()>;
    async fn read_response(&mut self, sent_command: &Command) -> io::Result<CommandResult>;
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
    fn new_fast_switching_antenna_iterator(
        &mut self,
        antenna_cfg: Vec<(u8, u8)>,
    ) -> impl Stream<Item = Result<Tag, ConnectorError>>;
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

    async fn read_response(&mut self, sent_command: &Command) -> io::Result<CommandResult> {
        let mut buffer = BytesMut::with_capacity(1024);
        let mut temp = [0u8; 1024];

        loop {
            match self.socket.read(&mut temp).await {
                Ok(n) if n > 0 => {
                    buffer.extend_from_slice(&temp[..n]);
                    if let Some(o) = try_parsing_results(Vec::from(buffer.clone()), sent_command) {
                        debug_print_vec("RX", &buffer);
                        return Ok(o);
                    }
                }
                Ok(_) => {
                    debug!("EOF - dispositivo probabilmente disconnesso");
                    // n == 0 → porta chiusa
                    break;
                }
                Err(e) => {
                    error!("Error reading from socket: {e}");
                    return Err(e);
                }
            }
        }

        Err(io::Error::new(
            io::ErrorKind::Other,
            "Timeout waiting for response",
        ))
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
        match self.read_command(&cmd).await {
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
                self.send_and_read_command(cmd).await
            }
            Err(e) => Err(e),
        }
    }

    async fn read_command(
        &mut self,
        sent_command: &Command,
    ) -> Result<CommandResult, ConnectorError> {
        timed_debug!(
            "Response time:",
            match tokio::time::timeout(
                self.timeout_waiting_packet,
                self.read_response(sent_command),
            )
            .await
            {
                Ok(Ok(res)) => Ok(res),
                Ok(Err(e)) => Err(ConnectorError::Io(e)),
                Err(_) => Err(ConnectorError::Timeout),
            }
        )
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

        if let CommandResult::GetFrequencyRegion(Ok(region)) = response {
            if region.0 != p0 || region.1 != p1 || region.2 != p2 {
                debug!("NEED CHANGE FREQUENCY REGION: {} {} {}", p0, p1, p2);
                self.send_and_read_command(Command::SetDefaultFrequencyRegion(p0, p1, p2))
                    .await?;
            }
            Ok(())
        } else {
            Err(ConnectorError::FailedSetting(format!(
                "Failed to check Frequency Region for new settings {:?} {:?} {:?}",
                p0, p1, p2
            )))
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
        let response = self
            .send_and_read_command(Command::GetAntConnectionDetector)
            .await?;

        if let CommandResult::GetAntConnectionDetector(Ok(setted_values)) = response {
            if setted_values != p0 {
                debug!("NEED CHANGE ConnectionDetector value: {:?}", p0);
                self.send_and_read_command(Command::SetAntConnectionDetector(p0.clone()))
                    .await?;
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
        let response = self
            .send_and_read_command(Command::GetRfLinkProfile)
            .await?;
        if let CommandResult::GetRfLinkProfile(Ok(setted_values)) = response {
            if setted_values != p0 {
                debug!("NEED CHANGE RfLinkProfile to value: {:?}", p0);
                self.send_and_read_command(Command::SetRfLinkProfile(p0.clone()))
                    .await?;
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
        let response = self
            .send_and_read_command(Command::CustomizeSessionTargetInventory(
                Session::S1,
                Target::A,
                PhaseStatus::Off,
                1,
            ))
            .await?;
        debug!("Risposta ricevuta: {response}\n");

        if let CommandResult::ResponsePackets(Ok(setted_values)) = response {
            debug!("{:?}", setted_values);
            Ok(setted_values.0)
        } else {
            Err(ConnectorError::TagReadError(format!("Failed to read Tags")))
        }
    }

    fn new_fast_switching_antenna_iterator(
        &mut self,
        antenna_cfg: Vec<(u8, u8)>,
    ) -> impl Stream<Item = Result<Tag, ConnectorError>> {
        let cmd = Command::FastSwitchAntInventory(
            antenna_cfg,
            0,
            Session::S1,
            Target::A,
            PhaseStatus::Off,
            1,
        );

        let iter_tag = tag_stream_async(self, cmd);

        // let iter_tag = tag_iterator::tag_stream(self, cmd, std::time::Duration::from_secs(0));

        iter_tag
    }
}

/// Si occupa di controllare se abbiamo ricevuto tutti i byte per la comunicazione
fn try_parsing_results(buf: Vec<u8>, sent_command: &Command) -> Option<CommandResult> {
    match Command::from_byte(buf, sent_command) {
        Ok(o) => {
            debug!("Ricevuto tutti i byte per la comunicazione");
            Some(o)
        }
        _ => None,
    }
}

#[cfg(all(test, feature = "async"))]
mod tests {
    use super::*;
    use std::io;
    use std::pin::Pin;
    use std::task::{Context, Poll};
    use tokio::io::{AsyncRead, AsyncWrite};

    /// Mock asincrono per simulare una socket che implementa AsyncRead/AsyncWrite
    struct AsyncMockSocket {
        // Sequenza di risultati di lettura: ogni elemento rappresenta un "chunk" letto
        read_data: Vec<Result<Vec<u8>, io::Error>>,
        read_index: usize,
        written: Vec<u8>,
    }

    impl AsyncMockSocket {
        fn new(read_data: Vec<Result<Vec<u8>, io::Error>>) -> Self {
            Self {
                read_data,
                read_index: 0,
                written: Vec::new(),
            }
        }

        fn written(&self) -> &[u8] {
            &self.written
        }
    }

    impl AsyncRead for AsyncMockSocket {
        fn poll_read(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &mut tokio::io::ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            let me = self.get_mut();

            if me.read_index >= me.read_data.len() {
                // EOF immediato
                return Poll::Ready(Ok(()));
            }

            match &me.read_data[me.read_index] {
                Ok(data) => {
                    let to_copy = data.len().min(buf.remaining());
                    buf.put_slice(&data[..to_copy]);
                    me.read_index += 1;
                    Poll::Ready(Ok(()))
                }
                Err(e) => {
                    let kind = e.kind();
                    me.read_index += 1;
                    Poll::Ready(Err(io::Error::new(kind, "mock async read error")))
                }
            }
        }
    }

    impl AsyncWrite for AsyncMockSocket {
        fn poll_write(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            let me = self.get_mut();
            me.written.extend_from_slice(buf);
            Poll::Ready(Ok(buf.len()))
        }

        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    fn new_connector_with(socket: AsyncMockSocket) -> Connector<AsyncMockSocket> {
        Connector::new(
            socket,
            1,
            vec![30],
            (crate::frequency_references::Spectrum::CHN, 920.125, 924.875),
            None,
        )
    }

    #[tokio::test]
    async fn test_async_send_frame_writes_bytes() {
        let socket = AsyncMockSocket::new(vec![]);
        let mut connector = new_connector_with(socket);

        let data = vec![0xAA, 0xBB, 0xCC, 0xDD];
        connector
            .send_frame(&data)
            .await
            .expect("send_frame fallita");

        let written = connector.into_inner().written().to_vec();
        assert_eq!(written, data);
    }
}
