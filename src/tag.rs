use crate::frequency_references::get_frequency;

/// Rappresentazione di un tag
#[derive(Debug, Clone)]
pub struct Tag {
    pub frequency: f64,
    pub antenna_id: u8,
    pub epc: String,
    pub pc: String,
    pub rssi: u8,
    pub phase: (u8, u8),
    raw: Vec<u8>,
}

impl Tag {
    pub(crate) fn from_raw_with_phase(raw: Vec<u8>) -> Tag {

        let antenna_id: u8 = raw[0] & 0b0000_0011; // low 2 bits
        let frequency: u8 = raw[0] >> 2;    // high 6 bits

        Self {
            frequency: get_frequency(frequency),
            pc: bytes_to_hex_upper(&raw[1..3].to_vec()),
            epc: bytes_to_hex_upper(&raw[3..raw.len()-3]),
            phase: (raw[raw.len()-3], raw[raw.len()-2]),
            rssi: raw[raw.len()-1],
            raw,
            antenna_id
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

#[cfg(test)]
mod tests {
    use crate::tag::Tag;

    #[test]
    fn check_from_raw_with_phase() {
        let raw = vec![
            0x13, //FreqAnt
            0x30, 0x00, //PC
            0xE2, 0x80, 0x69, 0x15, 0x00, 0x00, 0x40, 0x1D, 0x63, 0xE3, 0x28, 0x4F, // EPC
            0x09,0x10, // Phase
            0xC6,
        ];
        let tag = Tag::from_raw_with_phase(raw.clone());
        assert_eq!(tag.frequency, 867.0);
        assert_eq!(tag.antenna_id, 3);
        assert_eq!(tag.pc, "3000");
        assert_eq!(tag.epc,"E28069150000401D63E3284F");
        assert_eq!(tag.phase, (0x9,0x10));
        assert_eq!(tag.rssi, 0xC6);
        assert_eq!(tag.raw, raw);
    }
}
