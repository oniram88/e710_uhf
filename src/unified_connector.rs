#[cfg(feature = "async")]
mod async_impl;
mod sync;

#[cfg(feature = "async")]
pub use async_impl::*;

use std::io::{self, Read, Write};

/// Connettore unificato
pub struct UnifiedConnector<S> {
    socket: S,
}

impl<S> UnifiedConnector<S> {
    pub fn new(socket: S) -> Self {
        UnifiedConnector { socket }
    }
    pub fn into_inner(self) -> S {
        self.socket
    }
}
