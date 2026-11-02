//! Private JSON adapters that preserve object shape and required nullable presence.
use serde::{
    Deserialize, Deserializer,
    de::{MapAccess, Visitor, value::MapAccessDeserializer},
};
use std::{fmt, marker::PhantomData};

pub(crate) struct Object<T>(pub T);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Object<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ObjectVisitor<T>(PhantomData<T>);
        impl<'de, T: Deserialize<'de>> Visitor<'de> for ObjectVisitor<T> {
            type Value = Object<T>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an object")
            }
            fn visit_map<M: MapAccess<'de>>(self, map: M) -> Result<Self::Value, M::Error> {
                T::deserialize(MapAccessDeserializer::new(map)).map(Object)
            }
        }
        deserializer.deserialize_map(ObjectVisitor(PhantomData))
    }
}

pub(crate) struct NullableString(pub Option<String>);

pub(crate) struct UnsignedToken(pub u64);

impl<'de> Deserialize<'de> for UnsignedToken {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct UnsignedVisitor;
        impl Visitor<'_> for UnsignedVisitor {
            type Value = UnsignedToken;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an unsigned integer token")
            }
            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
                Ok(UnsignedToken(value))
            }
        }
        // Typed deserialize_u64 takes serde_json's machine-number conversion
        // path even with arbitrary_precision. deserialize_any instead preserves
        // arbitrary valid number tokens; this visitor rejects floats/negatives
        // and the private arbitrary-number map as data/type errors.
        deserializer.deserialize_any(UnsignedVisitor)
    }
}

impl<'de> Deserialize<'de> for NullableString {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct NullableVisitor;
        impl Visitor<'_> for NullableVisitor {
            type Value = NullableString;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("explicit null or string")
            }
            fn visit_unit<E>(self) -> Result<Self::Value, E> {
                Ok(NullableString(None))
            }
            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
                Ok(NullableString(Some(value.to_owned())))
            }
            fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
                Ok(NullableString(Some(value)))
            }
        }
        // deserialize_option would also accept serde's missing-field sentinel.
        deserializer.deserialize_any(NullableVisitor)
    }
}

pub(crate) fn decimal_u64(value: &str) -> Option<u64> {
    if value == "0" {
        return Some(0);
    }
    let bytes = value.as_bytes();
    if !matches!(bytes.first(), Some(b'1'..=b'9')) || !bytes.iter().all(u8::is_ascii_digit) {
        return None;
    }
    value.parse().ok()
}

pub(crate) fn nullable_decimal(value: NullableString) -> Result<Option<u64>, ()> {
    value.0.map(|s| decimal_u64(&s).ok_or(())).transpose()
}

pub(crate) fn json_optional_u64(value: Option<u64>) -> String {
    value.map_or_else(|| "null".to_owned(), |n| format!("\"{n}\""))
}
