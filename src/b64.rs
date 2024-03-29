//! functions that help the conversion between integers and pinv-style base64
//! strings.

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
use simple_error::bail;
use std::error::Error;

/// Table containing all the numerals for pinv-style base64. Their position in
/// the table determines their value e.g. "A" is at index 10 and has a value of
/// 10 & "+" has an index of 62 and has a value of 62
static TABLE: [char; 64] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I',
    'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', 'a', 'b',
    'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u',
    'v', 'w', 'x', 'y', 'z', '+', '-',
];

/// Takes a u64 and converts it to a pinv-style base64 string
pub fn from_u64(num: u64) -> String {
    let mut out = String::new();

    let mut num = num;

    let mut i = 64;

    // If the number is zero we don't need to do anything
    if num == 0 {
        return "0".to_string();
    }

    while num > 0 {
        let j = num % i;

        out.push(TABLE[j as usize]);

        num /= i;
        i *= 64;
    }

    // Return the reversed string since we built it backwards(to be more effecient)
    out.chars().rev().collect::<String>()
}

/// Takes a pinv-style base64 string and converts it to a u64. Unwraps on
/// error or invalid character, should be changed in an update.
pub fn to_u64(string: &str) -> Result<u64, Box<dyn Error>> {
    let mut pow = 1;
    let mut out: u64 = 0;

    for digit in string.trim().chars().rev() {
        let digit_val = match TABLE.iter().position(|x| x == &digit) {
            Some(digit_val) => digit_val,
            None => {
                bail!("Invalid digit {}!", digit);
            }
        };

        out += (digit_val as u64) * pow;

        pow *= 64;
    }

    Ok(out)
}
