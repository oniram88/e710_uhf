use crate::connector::{Connector, ConnectorError};
use crate::frame::command::{Command, CommandResult};
use crate::tag::Tag;
use log::{debug, error};
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::time::{Duration, Instant};

pub struct TagIterator<'a, P: Read + Write> {
    connector: &'a mut Connector<P>,
    sent_command: &'a Command,
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

        match self.connector.read_command(self.sent_command) {
            Ok(response) => {
                debug!("Risposta ricevuta: {response}\n");
                if let CommandResult::ResponsePackets(Ok(setted_values)) = response {
                    self.buffer.extend(setted_values.0);
                    self.finished = true;
                }
            }
            Err(e) => {
                error!("Risposta ricevuta con errori {e}");
            }
        }
        self.buffer.pop_front().map(Ok)
    }
}

pub(crate) fn tag_stream<'a, P: std::io::Read + std::io::Write>(
    connector: &'a mut Connector<P>,
    sent_command: &'a Command,
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
