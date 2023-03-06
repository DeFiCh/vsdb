//!
//! # ende
//!
//! (en)Encode and (de)Decode
//!

use super::RawBytes;
use ruc::*;
use serde::{de::DeserializeOwned, Serialize};
use std::{
    fmt,
    mem::{size_of, transmute},
};

/////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////

/// Methods used to encode the KEY.
pub trait KeyEn: Serialize + Sized {
    /// Encode original key type to bytes.
    #[cfg(feature = "json_codec")]
    fn try_encode_key(&self) -> Result<RawBytes> {
        serde_json::to_vec(self).c(d!())
    }

    /// Encode original key type to bytes.
    #[cfg(feature = "bcs_codec")]
    fn try_encode_key(&self) -> Result<RawBytes> {
        bcs::to_bytes(self).c(d!())
    }

    /// Encode original key type to bytes.
    #[cfg(feature = "msgpack_codec")]
    fn try_encode_key(&self) -> Result<RawBytes> {
        rmp_serde::to_vec(self).c(d!())
    }

    fn encode_key(&self) -> RawBytes {
        pnk!(self.try_encode_key())
    }
}

/// Methods used to decode the KEY.
pub trait KeyDe: DeserializeOwned {
    /// Decode from bytes to the original key type.
    #[cfg(feature = "json_codec")]
    fn decode_key(bytes: &[u8]) -> Result<Self> {
        serde_json::from_slice(bytes).c(d!())
    }

    /// Decode from bytes to the original key type.
    #[cfg(feature = "bcs_codec")]
    fn decode_key(bytes: &[u8]) -> Result<Self> {
        bcs::from_bytes(bytes).c(d!())
    }

    /// Decode from bytes to the original key type.
    #[cfg(feature = "msgpack_codec")]
    fn decode_key(bytes: &[u8]) -> Result<Self> {
        rmp_serde::from_slice(bytes).c(d!())
    }
}

/// Methods used to encode and decode the KEY.
pub trait KeyEnDe: KeyEn + KeyDe {
    /// Encode original key type to bytes.
    fn try_encode(&self) -> Result<RawBytes> {
        <Self as KeyEn>::try_encode_key(self).c(d!())
    }

    fn encode(&self) -> RawBytes {
        pnk!(self.try_encode())
    }

    /// Decode from bytes to the original key type.
    fn decode(bytes: &[u8]) -> Result<Self> {
        <Self as KeyDe>::decode_key(bytes).c(d!())
    }
}

/// Methods used to encode the VALUE.
pub trait ValueEn: Serialize + Sized {
    /// Encode original key type to bytes.
    #[cfg(feature = "json_codec")]
    fn try_encode_value(&self) -> Result<RawBytes> {
        serde_json::to_vec(self).c(d!())
    }

    /// Encode original key type to bytes.
    #[cfg(feature = "bcs_codec")]
    fn try_encode_value(&self) -> Result<RawBytes> {
        bcs::to_bytes(self).c(d!())
    }

    /// Encode original key type to bytes.
    #[cfg(feature = "msgpack_codec")]
    fn try_encode_value(&self) -> Result<RawBytes> {
        rmp_serde::to_vec(self).c(d!())
    }

    fn encode_value(&self) -> RawBytes {
        pnk!(self.try_encode_value())
    }
}

/// Methods used to decode the VALUE.
pub trait ValueDe: DeserializeOwned {
    /// Decode from bytes to the original key type.
    #[cfg(feature = "json_codec")]
    fn decode_value(bytes: &[u8]) -> Result<Self> {
        serde_json::from_slice(bytes).c(d!())
    }

    /// Decode from bytes to the original key type.
    #[cfg(feature = "bcs_codec")]
    fn decode_value(bytes: &[u8]) -> Result<Self> {
        bcs::from_bytes(bytes).c(d!())
    }

    /// Decode from bytes to the original key type.
    #[cfg(feature = "msgpack_codec")]
    fn decode_value(bytes: &[u8]) -> Result<Self> {
        rmp_serde::from_slice(bytes).c(d!())
    }
}

/// Methods used to encode and decode the VALUE.
pub trait ValueEnDe: ValueEn + ValueDe {
    /// Encode original key type to bytes.
    fn try_encode(&self) -> Result<RawBytes> {
        <Self as ValueEn>::try_encode_value(self).c(d!())
    }

    fn encode(&self) -> RawBytes {
        pnk!(self.try_encode())
    }

    /// Decode from bytes to the original key type.
    fn decode(bytes: &[u8]) -> Result<Self> {
        <Self as ValueDe>::decode_value(bytes).c(d!())
    }
}

impl<T: Serialize> KeyEn for T {}
impl<T: Serialize> ValueEn for T {}

impl<T: DeserializeOwned> KeyDe for T {}
impl<T: DeserializeOwned> ValueDe for T {}

impl<T: KeyEn + KeyDe> KeyEnDe for T {}
impl<T: ValueEn + ValueDe> ValueEnDe for T {}

/////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////

/// For keys that their serialized order keep consistent with their original format.
/// When using this kind of keys, we can do some ordered operations, such as: `get_le/get_be ...`
pub trait KeyEnDeOrdered: Clone + Eq + Ord + fmt::Debug {
    /// &key => bytes
    fn to_bytes(&self) -> RawBytes;

    /// key => bytes
    fn into_bytes(self) -> RawBytes {
        self.to_bytes()
    }

    /// &bytes => key
    fn from_slice(b: &[u8]) -> Result<Self>;

    /// bytes => key
    fn from_bytes(b: RawBytes) -> Result<Self> {
        Self::from_slice(&b)
    }
}

impl KeyEnDeOrdered for RawBytes {
    #[inline(always)]
    fn to_bytes(&self) -> RawBytes {
        self.clone()
    }

    #[inline(always)]
    fn into_bytes(self) -> RawBytes {
        self
    }

    #[inline(always)]
    fn from_slice(b: &[u8]) -> Result<Self> {
        Ok(b.to_vec())
    }

    #[inline(always)]
    fn from_bytes(b: RawBytes) -> Result<Self> {
        Ok(b.to_vec())
    }
}

impl KeyEnDeOrdered for Box<[u8]> {
    #[inline(always)]
    fn to_bytes(&self) -> RawBytes {
        self.clone().to_vec()
    }

    #[inline(always)]
    fn into_bytes(self) -> RawBytes {
        self.to_vec()
    }

    #[inline(always)]
    fn from_slice(b: &[u8]) -> Result<Self> {
        Ok(b.to_vec().into())
    }

    #[inline(always)]
    fn from_bytes(b: RawBytes) -> Result<Self> {
        Ok(b.into())
    }
}

impl KeyEnDeOrdered for String {
    #[inline(always)]
    fn to_bytes(&self) -> RawBytes {
        self.as_bytes().to_vec()
    }

    #[inline(always)]
    fn into_bytes(self) -> RawBytes {
        self.into_bytes()
    }

    #[inline(always)]
    fn from_slice(b: &[u8]) -> Result<Self> {
        String::from_utf8(b.to_owned()).c(d!())
    }

    #[inline(always)]
    fn from_bytes(b: RawBytes) -> Result<Self> {
        String::from_utf8(b).c(d!())
    }
}

macro_rules! impl_type {
    ($int: ty) => {
        impl KeyEnDeOrdered for $int {
            #[inline(always)]
            fn to_bytes(&self) -> RawBytes {
                self.to_be_bytes().to_vec()
            }
            #[inline(always)]
            fn from_slice(b: &[u8]) -> Result<Self> {
                <[u8; size_of::<$int>()]>::try_from(b)
                    .c(d!())
                    .map(<$int>::from_be_bytes)
            }
        }
    };
    (@$int: ty) => {
        #[allow(clippy::unsound_collection_transmute)]
        impl KeyEnDeOrdered for Vec<$int> {
            #[inline(always)]
            fn to_bytes(&self) -> RawBytes {
                self.iter()
                    .map(|i| i.to_be_bytes())
                    .flatten()
                    .collect::<Vec<_>>()
            }
            #[inline(always)]
            fn into_bytes(mut self) -> RawBytes {
                for i in 0..self.len() {
                    self[i] = self[i].to_be();
                }
                unsafe {
                    let v = transmute::<Vec<$int>, RawBytes>(self);
                    v
                }
            }
            #[inline(always)]
            fn from_slice(b: &[u8]) -> Result<Self> {
                if 0 != b.len() % size_of::<$int>() {
                    return Err(eg!("invalid bytes"));
                }
                b.chunks(size_of::<$int>())
                    .map(|i| {
                        <[u8; size_of::<$int>()]>::try_from(i)
                            .c(d!())
                            .map(<$int>::from_be_bytes)
                    })
                    .collect()
            }
            #[inline(always)]
            fn from_bytes(b: RawBytes) -> Result<Self> {
                if 0 != b.len() % size_of::<$int>() {
                    return Err(eg!("invalid bytes"));
                }
                let mut ret = unsafe {
                    let mut v = transmute::<Vec<u8>, Vec<$int>>(b);
                    v.set_len(v.len() / size_of::<$int>());
                    v
                };
                for i in 0..ret.len() {
                    ret[i] = <$int>::from_be(ret[i]);
                }
                Ok(ret)
            }
        }
    };
    (^$int: ty) => {
        impl KeyEnDeOrdered for Box<[$int]> {
            #[inline(always)]
            fn to_bytes(&self) -> RawBytes {
                KeyEnDeOrdered::to_bytes(&self.to_vec())
            }
            #[inline(always)]
            fn into_bytes(self) -> RawBytes {
                KeyEnDeOrdered::into_bytes(self.to_vec())
            }
            #[inline(always)]
            fn from_slice(b: &[u8]) -> Result<Self> {
                <Vec<$int> as KeyEnDeOrdered>::from_slice(b).map(|b| b.into())
            }
            #[inline(always)]
            fn from_bytes(b: RawBytes) -> Result<Self> {
                <Vec<$int> as KeyEnDeOrdered>::from_bytes(b).map(|b| b.into())
            }
        }
    };
    ($int: ty, $siz: expr) => {
        impl KeyEnDeOrdered for [$int; $siz] {
            #[inline(always)]
            fn to_bytes(&self) -> RawBytes {
                self.iter()
                    .map(|i| i.to_be_bytes())
                    .flatten()
                    .collect::<Vec<_>>()
            }
            #[inline(always)]
            fn from_slice(b: &[u8]) -> Result<Self> {
                if 0 != b.len() % size_of::<$int>() {
                    return Err(eg!("invalid bytes"));
                }
                if $siz != b.len() / size_of::<$int>() {
                    return Err(eg!("invalid bytes"));
                }
                let mut res = [0; $siz];
                b.chunks(size_of::<$int>())
                    .enumerate()
                    .for_each(|(idx, i)| {
                        res[idx] = <[u8; size_of::<$int>()]>::try_from(i)
                            .map(<$int>::from_be_bytes)
                            .unwrap();
                    });
                Ok(res)
            }
        }
    };
}

impl_type!(i8);
impl_type!(i16);
impl_type!(i32);
impl_type!(i64);
impl_type!(i128);
impl_type!(isize);
impl_type!(u8);
impl_type!(u16);
impl_type!(u32);
impl_type!(u64);
impl_type!(u128);
impl_type!(usize);

impl_type!(@i8);
impl_type!(@i16);
impl_type!(@i32);
impl_type!(@i64);
impl_type!(@i128);
impl_type!(@isize);
// impl_type!(@u8);
impl_type!(@u16);
impl_type!(@u32);
impl_type!(@u64);
impl_type!(@u128);
impl_type!(@usize);

impl_type!(^i8);
impl_type!(^i16);
impl_type!(^i32);
impl_type!(^i64);
impl_type!(^i128);
impl_type!(^isize);
// impl_type!(^u8);
impl_type!(^u16);
impl_type!(^u32);
impl_type!(^u64);
impl_type!(^u128);
impl_type!(^usize);

macro_rules! impl_repeat {
    ($i: expr) => {
        impl_type!(i8, $i);
        impl_type!(i16, $i);
        impl_type!(i32, $i);
        impl_type!(i64, $i);
        impl_type!(i128, $i);
        impl_type!(isize, $i);
        impl_type!(u8, $i);
        impl_type!(u16, $i);
        impl_type!(u32, $i);
        impl_type!(u64, $i);
        impl_type!(u128, $i);
        impl_type!(usize, $i);
    };
    ($i: expr, $($ii: expr),+) => {
        impl_repeat!($i);
        impl_repeat!($($ii), +);
    };
}

impl_repeat!(
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
    24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44,
    45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65,
    66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86,
    87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105,
    106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122,
    123, 124, 125, 126, 127, 128
);

/////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////
