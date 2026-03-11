use crate::frequency_references::get_frequency;
use std::fmt::Display;
use std::time::{SystemTime, UNIX_EPOCH};
use chrono::{DateTime, Utc};

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
    pub received_at_utc: DateTime<Utc>,
    pub antenna_choosing: Option<u8>, // When 0, take antenna 1/2/3/4; When 1, take antenna 5/6/7/8
}

impl Tag {
    pub(crate) fn from_raw_with_phase(raw: &[u8]) -> Tag {
        let (antenna_id, frequency) = Self::extract_fre_ant_id(&raw[0]);

        let (rssi, antenna_choosing) = Self::extract_rssi_choosing_antenna(&raw[raw.len() - 3]);

        Self {
            frequency: get_frequency(frequency),
            pc: bytes_to_hex_upper(&raw[1..3].to_vec()),
            epc: bytes_to_hex_upper(&raw[3..raw.len() - 3]),
            phase: (raw[raw.len() - 2], raw[raw.len() - 1]),
            rssi,
            antenna_id,
            antenna_choosing,
            received_at_utc: Utc::now(),
            received_at_ns: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_nanos() as u64,
        }
    }

    fn extract_fre_ant_id(freq_ant_byte: &u8) -> (u8, u8) {
        let antenna_id: u8 = freq_ant_byte & 0b0000_0011; // low 2 bits
        let frequency: u8 = freq_ant_byte >> 2; // high 6 bits

        (antenna_id, frequency)
    }

    fn extract_rssi_choosing_antenna(rssi_byte: &u8) -> (u8, Option<u8>) {
        let rssi: u8;
        let antenna_choosing: Option<u8>;
        if *rssi_byte != 0x00 {
            rssi = rssi_byte & 0b0111_1111; // low 7 bits
            antenna_choosing = Some(rssi_byte >> 7); // high 1 bit
        } else {
            rssi = 0;
            antenna_choosing = None;
        }
        (rssi, antenna_choosing)
    }

    pub(crate) fn from_raw(raw: &[u8]) -> Tag {
        let (antenna_id, frequency) = Self::extract_fre_ant_id(&raw[0]);
        let (rssi, antenna_choosing) = Self::extract_rssi_choosing_antenna(&raw[raw.len() - 1]);

        Self {
            frequency: get_frequency(frequency),
            pc: bytes_to_hex_upper(&raw[1..3].to_vec()),
            epc: bytes_to_hex_upper(&raw[3..raw.len() - 1]),
            phase: (0, 0),
            rssi,
            antenna_id,
            antenna_choosing,
            received_at_utc: Utc::now(),
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
        write!(
            f,
            "[{}] [{}] | {} | RSSI:{}",
            self.received_at_ns, self.antenna_id, self.epc, self.rssi
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::tag::Tag;

    #[test]
    fn check_from_raw_with_phase() {
        let raw = vec![
            0x00, //FreqAnt
            0x30, 0x00, // PC
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x15, 0x22, // EPC
            0x54, // RSSI 01010100
            0x00, 0x32, // Phase
        ];
        let tag = Tag::from_raw_with_phase(&*raw);
        assert_eq!(tag.frequency, 865.0);
        assert_eq!(tag.antenna_id, 0);
        assert_eq!(tag.pc, "3000");
        assert_eq!(tag.epc, "000000000000000000001522");
        assert_eq!(tag.phase, (0x00, 0x32));
        assert_eq!(tag.rssi, 0x54);
        assert_eq!(tag.antenna_choosing, Some(0));
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
        assert_eq!(tag.rssi, 0x46);
        assert_eq!(tag.antenna_choosing, Some(1));
    }

    #[test]
    fn test_extract_rssi_choosing_antenna() {
        let (rssi, antenna_choosing) = Tag::extract_rssi_choosing_antenna(&0xD4);

        assert_eq!(rssi, 0x54);
        assert_eq!(antenna_choosing, Some(1));

        let (_rssi, _antenna_choosing) = Tag::extract_rssi_choosing_antenna(&0x54);
        assert_eq!(_rssi, 0x54);
        assert_eq!(_antenna_choosing, Some(0));

        let (_rssi, _antenna_choosing) = Tag::extract_rssi_choosing_antenna(&0x00);
        assert_eq!(_rssi, 0x00);
        assert_eq!(_antenna_choosing, None);
    }
}
