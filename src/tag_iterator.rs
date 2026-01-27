use std::collections::VecDeque;
use std::io::{Read, Write};
use crate::tag::Tag;
use std::time::{Duration, Instant};
use log::{debug, error};
use crate::connector::{Connector, ConnectorError};
use crate::frame::CommandResult;

pub struct TagIterator<'a, P: Read + Write> {
    connector: &'a mut Connector<P>,
    counter: u32,
    last_emit: Instant,
    interval: Duration,
    buffer: VecDeque<Tag>
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

        // Rate limiting solo se impostato
        if self.interval > Duration::from_millis(0) {
            let elapsed = self.last_emit.elapsed();
            if elapsed < self.interval {
                std::thread::sleep(self.interval - elapsed);
            }
        }

        self.last_emit = Instant::now();

        match self.connector.read_command(){
            Ok(response) => {
                debug!("Risposta ricevuta: {response}\n");
                if let CommandResult::ResponsePackets(Ok(setted_values)) = response {
                    self.buffer.extend(setted_values.0);
                }
            }
            Err(e) => {
                error!("Risposta ricevuta con errori {e}");
            }
        }
        self.buffer
            .pop_front()
            .map(Ok)

    }
}

pub(crate) fn tag_stream<P: std::io::Read + std::io::Write>(connector: &mut Connector<P>, interval: Duration) -> TagIterator<P> {
    TagIterator {
        connector,
        counter: 0,
        last_emit: Instant::now(),
        buffer: VecDeque::new(),
        interval
    }
}