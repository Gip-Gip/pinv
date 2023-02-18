//! SVG Templates built-in to the pinv binary

use libflate::gzip::Decoder;
use std::io::Read;

pub struct Template {
    pub id: &'static str,
    pub data_compressed: &'static [u8],
}

impl Template {
    pub fn get_data(&self) -> Vec<u8> {
        let mut decoder = Decoder::new(self.data_compressed).unwrap();
        let mut out: Vec<u8> = Vec::new();

        decoder.read_to_end(&mut out).unwrap();

        out
    }
}

pub static TEMPLATES: [Template; 1] = [Template {
    id: "avery_18160",
    data_compressed: include_bytes!("../templates/avery_18160.svg.gz"),
}];
