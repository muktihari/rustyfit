static TABLE: [u16; 16] = [
    0x0000, 0xCC01, 0xD801, 0x1400, 0xF001, 0x3C00, 0x2800, 0xE401, 0xA001, 0x6C00, 0x7800, 0xB401,
    0x5000, 0x9C01, 0x8801, 0x4400,
];

pub(crate) struct Crc16(u16);

impl Crc16 {
    pub const fn new() -> Self {
        Self(0)
    }

    pub fn write(&mut self, b: &[u8]) {
        let mut crc = self.0;
        for &v in b {
            let mut tmp = TABLE[(crc as usize) & 0xF];
            crc = (crc >> 4) & 0x0FFF;
            crc = crc ^ tmp ^ TABLE[(v as usize) & 0xF];

            tmp = TABLE[(crc as usize) & 0xF];
            crc = (crc >> 4) & 0x0FFF;
            crc = crc ^ tmp ^ TABLE[((v as usize) >> 4) & 0xF];
        }
        self.0 = crc;
    }

    pub fn sum16(&self) -> u16 {
        self.0
    }

    pub fn reset(&mut self) {
        self.0 = 0
    }
}

#[cfg(test)]
mod tests {
    use crate::crc16::Crc16;

    #[test]
    fn test_all() {
        let b: [u8; 12] = [14, 32, 84, 8, 214, 204, 9, 0, 46, 70, 73, 84];
        let expected: u16 = 12856;

        let mut c = Crc16::new();
        c.write(&b);

        assert_eq!(c.sum16(), expected);

        c.reset();

        assert_eq!(c.sum16(), 0);
    }
}
