use std::io::{self, Read, Write};

pub struct Connector<P>
where
    P: Read + Write,
{
    port: P,
}

impl<P> Connector<P>
where
    P: Read + Write,
{
    pub fn new(p0: P) -> Self {
        Connector { port: p0 }
    }
}