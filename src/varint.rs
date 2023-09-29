/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 *
 * (c) ZeroTier, Inc.
 * https://www.zerotier.com/
 */

use std::io::{Read, Write};

pub const VARINT_MAX_SIZE_BYTES: usize = 10;

/// Encode an integer as a varint.
///
/// WARNING: if the supplied byte slice does not have at least 10 bytes available this may panic.
/// This is checked in debug mode by an assertion.
#[inline]
pub fn encode(b: &mut [u8], mut v: u64) -> usize {
    debug_assert!(b.len() >= VARINT_MAX_SIZE_BYTES);
    let mut i = 0;
    loop {
        if v > 0x7f {
            b[i] = (v as u8) & 0x7f;
            i += 1;
            v = v.wrapping_shr(7);
        } else {
            b[i] = (v as u8) | 0x80;
            i += 1;
            break;
        }
    }
    i
}

/// Write a variable length integer, which can consume up to 10 bytes.
#[inline]
pub fn write<W: Write>(w: &mut W, v: u64) -> std::io::Result<()> {
    let mut b = [0_u8; VARINT_MAX_SIZE_BYTES];
    let i = encode(&mut b, v);
    w.write_all(&b[0..i])
}

/// Dencode up to 10 bytes as a varint.
///
/// if the supplied byte slice does not contain a valid varint encoding this will return None.
/// if the supplied byte slice is shorter than expected this will return None.
#[inline]
pub fn decode(b: &[u8]) -> Option<(u64, usize)> {
    let mut v = 0_u64;
    let mut pos = 0;
    let mut i = 0_usize;
    while i < b.len() && i < VARINT_MAX_SIZE_BYTES {
        let b = b[i];
        i += 1;
        if b <= 0x7f {
            v |= (b as u64).wrapping_shl(pos);
            pos += 7;
        } else {
            v |= ((b & 0x7f) as u64).wrapping_shl(pos);
            return Some((v, i));
        }
    }
    None
}

/// Read a variable length integer, returning the value and the number of bytes written.
#[inline]
pub fn read<R: Read>(r: &mut R) -> std::io::Result<(u64, usize)> {
    let mut v = 0_u64;
    let mut buf = [0_u8; 1];
    let mut pos = 0;
    let mut i = 0_usize;
    loop {
        r.read_exact(&mut buf)?;
        let b = buf[0];
        i += 1;
        if b <= 0x7f {
            v |= (b as u64).wrapping_shl(pos);
            pos += 7;
        } else {
            v |= ((b & 0x7f) as u64).wrapping_shl(pos);
            return Ok((v, i));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::varint::*;

    #[test]
    fn varint() {
        let mut t: Vec<u8> = Vec::new();
        for i in 0..131072 {
            t.clear();
            let ii = (u64::MAX / 131072) * i;
            assert!(write(&mut t, ii).is_ok());
            let mut t2 = t.as_slice();
            assert_eq!(read(&mut t2).unwrap().0, ii);
        }
    }
}
