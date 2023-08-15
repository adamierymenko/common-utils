/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 *
 * (c) ZeroTier, Inc.
 * https://www.zerotier.com/
 */

use std::array::TryFromSliceError;
use std::fmt::Debug;
use std::hash::Hash;
use std::ops::Deref;
use std::ops::DerefMut;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use zeroize::Zeroize;

use crate::base64;
use crate::hex;

/// Fixed size Serde serializable byte array.
/// This makes it easier to deal with blobs larger than 32 bytes (due to serde array limitations)
#[repr(transparent)]
#[derive(Clone, Eq, PartialEq, PartialOrd, Ord, Hash, Zeroize)]
pub struct Blob<const L: usize>([u8; L]);

impl<const L: usize> Blob<L> {
    #[inline(always)]
    pub fn as_bytes(&self) -> &[u8; L] {
        &self.0
    }

    #[inline(always)]
    pub const fn len(&self) -> usize {
        L
    }
}

impl<const L: usize> DerefMut for Blob<L> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl<const L: usize> Deref for Blob<L> {
    type Target = [u8; L];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<const L: usize> From<Blob<L>> for [u8; L] {
    #[inline(always)]
    fn from(value: Blob<L>) -> Self {
        value.0
    }
}

impl<const L: usize> From<[u8; L]> for Blob<L> {
    #[inline(always)]
    fn from(a: [u8; L]) -> Self {
        Self(a)
    }
}

impl<const L: usize> From<&[u8; L]> for &Blob<L> {
    #[inline(always)]
    fn from(a: &[u8; L]) -> Self {
        // Blob is a transparent wrapper around an array, so this is just a type cast.
        unsafe { std::mem::transmute(a) }
    }
}

impl<const L: usize> TryFrom<&[u8]> for Blob<L> {
    type Error = TryFromSliceError;

    #[inline(always)]
    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        value.try_into().map(|b| Self(b))
    }
}

impl<const L: usize> Default for Blob<L> {
    #[inline(always)]
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

impl<const L: usize> AsRef<[u8; L]> for Blob<L> {
    #[inline(always)]
    fn as_ref(&self) -> &[u8; L] {
        &self.0
    }
}

impl<const L: usize> AsMut<[u8; L]> for Blob<L> {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut [u8; L] {
        &mut self.0
    }
}

impl<const L: usize> ToString for Blob<L> {
    #[inline(always)]
    fn to_string(&self) -> String {
        hex::to_string(&self.0)
    }
}

impl<const L: usize> Debug for Blob<L> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.to_string().as_str())
    }
}

impl<const L: usize> Serialize for Blob<L> {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            base64::to_string(&self.0).serialize(serializer)
        } else {
            serializer.serialize_bytes(&self.0)
        }
    }
}

struct BlobVisitor<const L: usize>;

impl<'de, const L: usize> serde::de::Visitor<'de> for BlobVisitor<L> {
    type Value = Blob<L>;

    #[inline]
    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str(format!("{} bytes", L).as_str())
    }

    #[inline]
    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let b = base64::from_string(v.trim().as_bytes()).ok_or(serde::de::Error::custom("invalid base64"))?;
        b.as_slice()
            .try_into()
            .map(|b| Blob::<L>(b))
            .map_err(|_| serde::de::Error::invalid_length(b.len(), &self))
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        v.try_into()
            .map(|b| Blob::<L>(b))
            .map_err(|_| serde::de::Error::invalid_length(v.len(), &self))
    }
}

impl<'de, const L: usize> Deserialize<'de> for Blob<L> {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            deserializer.deserialize_str(BlobVisitor::<L>)
        } else {
            deserializer.deserialize_bytes(BlobVisitor::<L>)
        }
    }
}
