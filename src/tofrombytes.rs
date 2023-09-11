/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 *
 * (c) ZeroTier, Inc.
 * https://www.zerotier.com/
 */

use std::io::{Read, Write};

use super::arrayvec::ArrayVec;

/// Trait for types that implement easy conversion to/from bytes either as slices or I/O streams.
/// The associated implement_serialize and implement_deserialize macros implement serde serialization
/// for types implementing both this and ToString / FromStr.
pub trait ToFromBytes: Sized {
    fn read_bytes<R: Read>(r: &mut R) -> std::io::Result<Self>;
    fn write_bytes<W: Write>(&self, w: &mut W) -> std::io::Result<()>;

    fn from_bytes(mut b: &[u8]) -> std::io::Result<Self> {
        Self::read_bytes(&mut b)
    }

    fn to_bytes(&self) -> Vec<u8> {
        let mut v = Vec::new();
        self.write_bytes(&mut v).expect("write_bytes() failed");
        v
    }

    fn to_bytes_on_stack<const BUF_SIZE: usize>(&self) -> ArrayVec<u8, BUF_SIZE> {
        let mut v = ArrayVec::new();
        self.write_bytes(&mut v).expect("write_bytes() failed");
        v
    }
}
