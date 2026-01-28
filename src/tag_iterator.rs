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
                }
            }
            Err(e) => {
                error!("Risposta ricevuta con errori {e}");
            }
        }
        self.finished = true;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frequency_references::Spectrum;
    use crate::frame::command::{Session, Target};
    use std::io::Cursor;

    #[test]
    fn test_tag_iterator_single_tag() {
        // Prepariamo una risposta con un singolo tag
        // A0 13 01 8B 17 30 00 E2 80 69 15 00 00 50 1D 63 E2 A0 4F D3 26 (tag)
        // A0 0A 01 8B 07 00 5A 00 00 00 01 ED (footer, 1 tag total)
        
        let mut raw_packet = vec![
            0xA0, 0x13, 0x01, 0x8B, 0x17, 0x30, 0x00, // head
            0xE2, 0x80, 0x69, 0x15, 0x00, 0x00, 0x50, 0x1D, 0x63, 0xE2, 0xA0, 0x4F, //EPC
            0xD3, 0x26, 
        ];
        raw_packet.extend(vec![
            0xA0, 0x0A, 0x01, 0x8B, // footer head
            0x07, // ant ID
            0x00, 0x5A, // ReadRate
            0x00, 0x00, 0x00, 0x01, // Total read = 1
            0x68, //Checksum
        ]);

        let cursor = Cursor::new(raw_packet);
        let mut connector = Connector::new(cursor, 1, vec![20], (Spectrum::ETSI, 865.0, 868.0));
        
        let cmd = Command::CustomizeSessionTargetInventory(Session::S0, Target::A, 0, 0);
        let mut iterator = tag_stream(&mut connector, &cmd, Duration::from_millis(0));
        
        let first = iterator.next();
        assert!(first.is_some());
        let tag = first.unwrap().unwrap();
        assert_eq!(tag.epc, "E28069150000501D63E2A04F");
        
        let second = iterator.next();
        assert!(second.is_none());
    }

    #[test]
    fn test_tag_iterator_multiple_tags() {
        let mut raw_packet = vec![
            0xA0, 0x13, 0x01, 0x8B, 0x17, 0x30, 0x00, 
            0xE2, 0x80, 0x69, 0x15, 0x00, 0x00, 0x50, 0x1D, 0x63, 0xE2, 0xA0, 0x4F, 
            0xD3, 0x26,
        ];
        raw_packet.extend(vec![
            0xA0, 0x13, 0x01, 0x8B, 0x17, 0x30, 0x00, 
            0xE2, 0x80, 0x69, 0x15, 0x00, 0x00, 0x40, 0x1D, 0x63, 0xE2, 0xA4, 0x4F, 
            0xD2, 0x33,
        ]);
        raw_packet.extend(vec![
            0xA0, 0x0A, 0x01, 0x8B, 
            0x07, 
            0x00, 0x5A, 
            0x00, 0x00, 0x00, 0x02, // Total read = 2
            0x67, // Checksum
        ]);

        let cursor = Cursor::new(raw_packet);
        let mut connector = Connector::new(cursor, 1, vec![20], (Spectrum::ETSI, 865.0, 868.0));
        
        let cmd = Command::CustomizeSessionTargetInventory(Session::S0, Target::A, 0, 0);
        let iterator = tag_stream(&mut connector, &cmd, Duration::from_millis(0));
        
        let tags: Vec<_> = iterator.collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].epc, "E28069150000501D63E2A04F");
        assert_eq!(tags[1].epc, "E28069150000401D63E2A44F");
    }

    #[test]
    fn test_tag_iterator_no_tags() {
        let raw_packet = vec![
            0xA0, 0x0A, 0x01, 0x8B, 
            0x07, 
            0x00, 0x5A, 
            0x00, 0x00, 0x00, 0x00, // Total read = 0
            0x69, // Checksum
        ];

        let cursor = Cursor::new(raw_packet);
        let mut connector = Connector::new(cursor, 1, vec![20], (Spectrum::ETSI, 865.0, 868.0));
        
        let cmd = Command::CustomizeSessionTargetInventory(Session::S0, Target::A, 0, 0);
        let mut iterator = tag_stream(&mut connector, &cmd, Duration::from_millis(0));
        
        let first = iterator.next();
        assert!(first.is_none());
    }
}
