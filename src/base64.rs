/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 *
 * (c) ZeroTier, Inc.
 * https://www.zerotier.com/
 */

use base64::{engine::general_purpose, Engine as _};

/// Encode a byte slice as Base64 using the URL-safe alphabet without padding
#[inline(always)]
pub fn to_string(b: &[u8]) -> String {
    general_purpose::URL_SAFE_NO_PAD.encode(b)
}

/// Decode a byte slcie using the URL-safe alphabet without padding
#[inline(always)]
pub fn from_string(s: &[u8]) -> Option<Vec<u8>> {
    general_purpose::URL_SAFE_NO_PAD.decode(s).ok()
}
