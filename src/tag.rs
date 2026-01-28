use std::fmt::Display;
use crate::frequency_references::get_frequency;
use std::time::{SystemTime, UNIX_EPOCH};

/// Rappresentazione di un tag
#[derive(Debug, Clone)]
pub struct Tag {
    pub frequency: f64,
    pub antenna_id: u8,
    pub epc: String,
    pub pc: String,
    pub rssi: u8,
    pub phase: (u8, u8),
    pub received_at_ns: u64,
}

impl Tag {
    pub(crate) fn from_raw_with_phase(raw: &[u8]) -> Tag {
        let antenna_id: u8 = raw[0] & 0b0000_0011; // low 2 bits
        let frequency: u8 = raw[0] >> 2; // high 6 bits

        Self {
            frequency: get_frequency(frequency),
            pc: bytes_to_hex_upper(&raw[1..3].to_vec()),
            epc: bytes_to_hex_upper(&raw[3..raw.len() - 3]),
            phase: (raw[raw.len() - 3], raw[raw.len() - 2]),
            rssi: raw[raw.len() - 1],
            antenna_id,
            received_at_ns: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_nanos() as u64,
        }
    }

    pub(crate) fn from_raw(raw: &[u8]) -> Tag {
        let antenna_id: u8 = raw[0] & 0b0000_0011; // low 2 bits
        let frequency: u8 = raw[0] >> 2; // high 6 bits

        Self {
            frequency: get_frequency(frequency),
            pc: bytes_to_hex_upper(&raw[1..3].to_vec()),
            epc: bytes_to_hex_upper(&raw[3..raw.len() - 1]),
            phase: (0, 0),
            rssi: raw[raw.len() - 1],
            antenna_id,
            received_at_ns: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_nanos() as u64,
        }
    }
}

fn bytes_to_hex_upper(bytes: &[u8]) -> String {
    // usa formatting manuale per performance / controllo
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02X}", b));
    }
    s
}

impl PartialEq<Self> for Tag {
    fn eq(&self, other: &Self) -> bool {
        self.epc == other.epc
    }
}

impl Display for Tag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] [{}] | {} | RSSI:{}",self.received_at_ns, self.antenna_id, self.epc, self.rssi)
    }
}

#[cfg(test)]
mod tests {
    use crate::tag::Tag;

    #[test]
    fn check_from_raw_with_phase() {
        let raw = vec![
            0x13, //FreqAnt
            0x30, 0x00, //PC
            0xE2, 0x80, 0x69, 0x15, 0x00, 0x00, 0x40, 0x1D, 0x63, 0xE3, 0x28, 0x4F, // EPC
            0x09, 0x10, // Phase
            0xC6,
        ];
        let tag = Tag::from_raw_with_phase(&*raw);
        assert_eq!(tag.frequency, 867.0);
        assert_eq!(tag.antenna_id, 3);
        assert_eq!(tag.pc, "3000");
        assert_eq!(tag.epc, "E28069150000401D63E3284F");
        assert_eq!(tag.phase, (0x9, 0x10));
        assert_eq!(tag.rssi, 0xC6);
    }

    #[test]
    fn check_from_raw() {
        let raw = vec![
            0x13, //FreqAnt
            0x30, 0x00, //PC
            0xE2, 0x80, 0x69, 0x15, 0x00, 0x00, 0x40, 0x1D, 0x63, 0xE3, 0x28, 0x4F, // EPC
            0xC6,
        ];
        let tag = Tag::from_raw(&*raw);
        assert_eq!(tag.frequency, 867.0);
        assert_eq!(tag.antenna_id, 3);
        assert_eq!(tag.pc, "3000");
        assert_eq!(tag.epc, "E28069150000401D63E3284F");
        assert_eq!(tag.phase, (0, 0));
        assert_eq!(tag.rssi, 0xC6);
    }
}
