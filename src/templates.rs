//! SVG Templates built-in to the pinv binary

// Copyright (c) 2023 Charles M. Thompson
//
// This file is part of pinv.
//
// pinv is free software: you can redistribute it and/or modify it under
// the terms only of version 3 of the GNU General Public License as published
// by the Free Software Foundation
//
// pinv is distributed in the hope that it will be useful, but WITHOUT ANY
// WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License
// for more details.
//
// You should have received a copy of the GNU General Public License along with
// pinv(in a file named COPYING).
// If not, see <https://www.gnu.org/licenses/>.
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
