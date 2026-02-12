use super::*;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use async_trait::async_trait;

#[async_trait]
pub trait AsyncIO {
    async fn write_async(&mut self, data: &[u8]) -> io::Result<usize>;
    async fn read_async(&mut self, buf: &mut [u8]) -> io::Result<usize>;
    async fn write_all_async(&mut self, data: &[u8]) -> io::Result<()>;
}

#[async_trait]
impl<S> AsyncIO for UnifiedConnector<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    async fn write_async(&mut self, data: &[u8]) -> io::Result<usize> {
        self.socket.write(data).await
    }

    async fn read_async(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.socket.read(buf).await
    }

    async fn write_all_async(&mut self, data: &[u8]) -> io::Result<()> {
        self.socket.write_all(data).await
    }
}