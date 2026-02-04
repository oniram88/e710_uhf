use crate::connector::{Connector, ConnectorError};
use crate::frame::command::{Command, CommandResult};
use crate::tag::Tag;
use log::{debug, error};
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::time::{Duration, Instant};

pub struct TagIterator<'a, P: Read + Write> {
    connector: &'a mut Connector<P>,
    sent_command: Command,
    last_emit: Instant,
    interval: Duration,
    buffer: VecDeque<Tag>,
    finished: bool, // identifica quando ho letto per intero un comando
}
impl<P> Iterator for TagIterator<'_, P>
where
    P: Read + Write,
{
    type Item = Result<Tag, ConnectorError>;

    fn next(&mut self) -> Option<Self::Item> {
        // Nel caso abbiamo bufferizzato multipli tags
        if let Some(tag) = self.buffer.pop_front() {
            return Some(Ok(tag));
        }

        if self.finished {
            return None;
        }

        // Rate limiting solo se impostato
        if self.interval > Duration::from_millis(0) {
            let elapsed = self.last_emit.elapsed();
            if elapsed < self.interval {
                std::thread::sleep(self.interval - elapsed);
            }
        }

        self.last_emit = Instant::now();
        match self.connector.send_command(&self.sent_command) {
            Ok(()) => match self.connector.read_command(&self.sent_command) {
                Ok(response) => {
                    debug!("Risposta ricevuta: {response}\n");
                    match response {
                        CommandResult::ResponsePackets(Ok(setted_values)) => {
                            self.buffer.extend(setted_values.0);
                            self.finished = true;
                        }
                        CommandResult::ResponsePackets(Err(e)) => {
                            self.finished = true;
                            return Some(Err(ConnectorError::from(e)));
                        }
                        _ => {
                            unreachable!();
                        }
                    }
                }
                Err(e) => {
                    self.finished = true;
                    return Some(Err(e));
                }
            },
            Err(e) => {
                error!("Errore inviando comando {e}");
                self.finished = true;
            }
        }

        self.buffer.pop_front().map(Ok)
    }
}

pub(crate) fn tag_stream<'a, P: std::io::Read + std::io::Write>(
    connector: &'a mut Connector<P>,
    sent_command: Command,
    interval: Duration,
) -> TagIterator<'a, P> {
    TagIterator {
        connector,
        sent_command,
        last_emit: Instant::now(),
        buffer: VecDeque::new(),
        interval,
        finished: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connector::Connector;
    use crate::frame::command::{Command, PhaseStatus, Session, Target};
    use crate::frequency_references::Spectrum;
    use std::collections::VecDeque;
    use std::io::{Read, Result, Write};
    use std::time::Duration;

    struct MockStream {
        read_data: VecDeque<u8>,
        written_data: Vec<u8>,
        should_fail_write: bool,
    }

    impl MockStream {
        fn new(read_data: Vec<u8>) -> Self {
            Self {
                read_data: VecDeque::from(read_data),
                written_data: Vec::new(),
                should_fail_write: false,
            }
        }

        fn new_failing_write() -> Self {
            Self {
                read_data: VecDeque::new(),
                written_data: Vec::new(),
                should_fail_write: true,
            }
        }
    }

    impl Read for MockStream {
        fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
            if self.read_data.is_empty() {
                return Ok(0);
            }
            let n = self.read_data.read(buf)?;
            Ok(n)
        }
    }

    impl Write for MockStream {
        fn write(&mut self, buf: &[u8]) -> Result<usize> {
            if self.should_fail_write {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Mock write error",
                ));
            }
            self.written_data.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn test_tag_iterator_success() {
        let mut raw_response = vec![
            0xA0, 0x13, 0x01, 0x8B, 0x17, 0x30, 0x00, // head
            0xE2, 0x80, 0x69, 0x15, 0x00, 0x00, 0x50, 0x1D, 0x63, 0xE2, 0xA0, 0x4F, //EPC 1
            0xD3, 0x26,
        ];
        raw_response.extend(vec![
            0xA0, 0x13, 0x01, 0x8B, 0x17, 0x30, 0x00, // head
            0xE2, 0x80, 0x69, 0x15, 0x00, 0x00, 0x40, 0x1D, 0x63, 0xE2, 0xA4, 0x4F, //EPC 2
            0xD2, 0x33,
        ]);
        raw_response.extend(vec![
            0xA0, 0x0A, 0x01, 0x8B, // footer
            0x07, // ant ID
            0x00, 0x5A, // ReadRate
            0x00, 0x00, 0x00, 0x02, // Total read (2)
            0x67, //Checksum
        ]);

        let stream = MockStream::new(raw_response);
        let mut connector = Connector::new(stream, 1, vec![20], (Spectrum::ETSI, 865.0, 868.0));

        let command =
            Command::CustomizeSessionTargetInventory(Session::S0, Target::A, PhaseStatus::Off, 0);

        let mut iterator = tag_stream(&mut connector, command, Duration::from_millis(0));

        let res1 = iterator.next().unwrap();
        assert!(res1.is_ok());
        assert_eq!(res1.unwrap().epc, "E28069150000501D63E2A04F");

        let res2 = iterator.next().unwrap();
        assert!(res2.is_ok());
        assert_eq!(res2.unwrap().epc, "E28069150000401D63E2A44F");

        assert!(iterator.next().is_none());
    }

    #[test]
    fn test_tag_iterator_send_error() {
        let stream = MockStream::new_failing_write();
        let mut connector = Connector::new(stream, 1, vec![20], (Spectrum::ETSI, 865.0, 868.0));

        let command = Command::GetWorkAntenna;
        let mut iterator = tag_stream(&mut connector, command, Duration::from_millis(0));

        assert!(iterator.next().is_none());
        assert!(iterator.finished);
    }

    #[test]
    fn test_tag_iterator_read_error() {
        // Risposta incompleta o errata
        let stream = MockStream::new(vec![0xA0, 0x01, 0x02]);
        let mut connector = Connector::new(stream, 1, vec![20], (Spectrum::ETSI, 865.0, 868.0));

        let command = Command::GetWorkAntenna;
        let mut iterator = tag_stream(&mut connector, command, Duration::from_millis(0));

        let res = iterator.next();
        assert!(res.is_some());
        assert!(res.unwrap().is_err());
        assert!(iterator.finished);
    }

    #[test]
    fn test_tag_iterator_no_tags_in_response() {
        // Una risposta di inventario che dice "0 tag trovati"
        let raw_response = vec![
            0xA0, 0x0A, 0x01, 0x8B, // footer
            0x07, // ant ID
            0x00, 0x00, // ReadRate
            0x00, 0x00, 0x00, 0x00, // Total read (0)
            0xC3, //Checksum
        ];

        let stream = MockStream::new(raw_response);
        let mut connector = Connector::new(stream, 1, vec![20], (Spectrum::ETSI, 865.0, 868.0));

        let command =
            Command::CustomizeSessionTargetInventory(Session::S0, Target::A, PhaseStatus::Off, 0);

        let mut iterator = tag_stream(&mut connector, command, Duration::from_millis(0));

        assert!(iterator.next().is_none());
    }

    #[test]
    fn test_tag_iterator_rate_limiting() {
        let raw_response = vec![
            0xA0, 0x0A, 0x01, 0x8B, 0x07, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC3,
        ];

        let stream = MockStream::new(raw_response);
        let mut connector = Connector::new(stream, 1, vec![20], (Spectrum::ETSI, 865.0, 868.0));

        let command =
            Command::CustomizeSessionTargetInventory(Session::S0, Target::A, PhaseStatus::Off, 0);

        let interval = Duration::from_millis(100);
        let mut iterator = tag_stream(&mut connector, command, interval);

        let start = std::time::Instant::now();
        let _ = iterator.next();
        let elapsed = start.elapsed();

        assert!(elapsed >= interval);
    }
}
