#![cfg(feature = "async")]

use crate::connector::{AsyncIO, Connector, ConnectorError};
use crate::frame::command::{Command, CommandResult};
use crate::tag::Tag;
use async_stream::try_stream;
use futures_core::stream::Stream;
use log::{debug, error, info, warn};
use tokio::io::{AsyncRead, AsyncWrite};

/// Restituisce uno Stream asincrono di `Tag` ottenuti eseguendo il `sent_command`.
///
/// - Usa i metodi asincroni del `Connector` per inviare e leggere la risposta.
/// - Emette `Result<Tag, ConnectorError>`; alla prima condizione d'errore lo stream termina
///   con l'errore (nessun retry implicito).
/// - `interval` permette un rate limiting tra invii successivi del comando.
pub fn tag_stream_async<'a, S>(
    connector: &'a mut Connector<S>,
    sent_command: Command,
) -> impl Stream<Item = Result<Tag, ConnectorError>> + 'a
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'a,
{
    try_stream! {

        if let Err(e) = connector.send_command(&sent_command).await {
            error!("Errore inviando comando: {:?}", e);
            Err::<(), ConnectorError>(e)?;
        }

        match connector.read_command(&sent_command).await {
            Ok(response) => {
                debug!("Risposta ricevuta: {:?}", response);
                match response {
                    CommandResult::ResponsePackets(Ok(setted_values)) => {
                        debug!("TAGS received:{:?}", setted_values.0.len());
                        for tag in setted_values.0 {
                            yield tag;
                        }
                    }
                    CommandResult::ResponsePackets(Err(e)) => {
                        Err(ConnectorError::from(e))?;
                    }
                    _ => {
                        // Coerente con l'implementazione sync: altri casi non dovrebbero verificarsi
                        unreachable!();
                    }
                }
            }
            Err(ConnectorError::Timeout) => {
                // In caso di Timeout facciamo un nuovo loop
                warn!("Timeout inviando comando: {:?}", sent_command);
            }
            Err(e) => {
                Err::<(), ConnectorError>(e)?;
            }
        }
    }
}
