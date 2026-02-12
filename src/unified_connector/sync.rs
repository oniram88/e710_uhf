use std::io;
use std::io::{Read, Write};
use crate::unified_connector::UnifiedConnector;

/// Trait per operazioni sincrone
pub trait SyncIO {
    fn write_sync(&mut self, data: &[u8]) -> io::Result<usize>;
    fn read_sync(&mut self, buf: &mut [u8]) -> io::Result<usize>;
    fn write_all_sync(&mut self, data: &[u8]) -> io::Result<()>;
}
/// Implementazione sincrona per qualsiasi socket che implementa Read + Write
impl<S> SyncIO for UnifiedConnector<S>
where
    S: Read + Write,
{
    fn write_sync(&mut self, data: &[u8]) -> io::Result<usize> {
        self.socket.write(data)
    }

    fn read_sync(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.socket.read(buf)
    }

    fn write_all_sync(&mut self, data: &[u8]) -> io::Result<()> {
        self.socket.write_all(data)
    }
}