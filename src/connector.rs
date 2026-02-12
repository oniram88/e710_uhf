#[cfg(feature = "async")]
mod async_impl;
pub mod sync;

#[cfg(feature = "async")]
pub use async_impl::*;


use crate::frame::{ FrameError};
use crate::frequency_references::Spectrum;
use std::fmt;
use std::io::{self};


pub struct Connector<S> {
    socket: S,
    total_number_of_antennas: u8,
    /// Potenza di lavoro da 0 a 33 db
    /// con un solo valore andremo ad impostare su tutte le antenne la medesima potenza
    /// con più valori ogni antenna avrà la sua potenza distinta
    output_power: Vec<u8>,
    working_freq_setup: (Spectrum, f64, f64),
}

impl<S> Connector<S> {
    pub fn new(socket: S,
               total_number_of_antennas: u8,
               output_power: Vec<u8>,
               working_freq_setup: (Spectrum, f64, f64)
    ) -> Self {
        Connector { socket, total_number_of_antennas: total_number_of_antennas,
            working_freq_setup,
            output_power, }
    }
    pub fn into_inner(self) -> S {
        self.socket
    }

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

const TIMEOUT_WAITING_PACKET: u64 = 150;



#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{self, Read, Write};
    use crate::connector::sync::SyncIO;

    struct MockPort {
        read_data: Vec<Result<Vec<u8>, io::Error>>,
        read_index: usize,
    }

    impl MockPort {
        fn new(read_data: Vec<Result<Vec<u8>, io::Error>>) -> Self {
            MockPort {
                read_data,
                read_index: 0,
            }
        }
    }

    impl Read for MockPort {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.read_index >= self.read_data.len() {
                return Ok(0);
            }

            match &self.read_data[self.read_index] {
                Ok(data) => {
                    let len = data.len();
                    buf[..len].copy_from_slice(data);
                    self.read_index += 1;
                    Ok(len)
                }
                Err(e) => {
                    // Cloniamo l'errore per poterlo restituire
                    let kind = e.kind();
                    self.read_index += 1;
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

    #[test]
    fn test_read_response_success() {
        let mock_port = MockPort::new(vec![Ok(vec![0x01, 0x02, 0x03])]);
        let mut connector =
            Connector::new(mock_port, 1, vec![30], (Spectrum::CHN, 920.125, 924.875));

        let response = connector.read_response().unwrap();
        assert_eq!(response, vec![0x01, 0x02, 0x03]);
    }

    #[test]
    fn test_read_response_empty() {
        let mock_port = MockPort::new(vec![]);
        let mut connector =
            Connector::new(mock_port, 1, vec![30], (Spectrum::CHN, 920.125, 924.875));

        let response = connector.read_response().unwrap();
        assert!(response.is_empty());
    }

    #[test]
    fn test_read_response_would_block() {
        let mock_port = MockPort::new(vec![
            Err(io::Error::new(io::ErrorKind::WouldBlock, "would block")),
            Ok(vec![0x04, 0x05]),
        ]);
        let mut connector =
            Connector::new(mock_port, 1, vec![30], (Spectrum::CHN, 920.125, 924.875));

        let response = connector.read_response().unwrap();
        assert_eq!(response, vec![0x04, 0x05]);
    }

    #[test]
    fn test_read_response_error() {
        let mock_port = MockPort::new(vec![Err(io::Error::new(
            io::ErrorKind::Other,
            "other error",
        ))]);
        let mut connector =
            Connector::new(mock_port, 1, vec![30], (Spectrum::CHN, 920.125, 924.875));

        let result = connector.read_response();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::Other);
    }
}
