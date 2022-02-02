use std::io::Cursor;

use bytes::Buf;
use serde::Deserialize;
use serde::de::{
    self, DeserializeSeed, EnumAccess, MapAccess, SeqAccess,
    VariantAccess, Visitor, DeserializeOwned,
};

use crate::error::{Error, Result};

pub struct Deserializer<'de> {
    input: Cursor<&'de [u8]>,
    le: bool
}

impl<'de> Deserializer<'de> {
   
    pub fn from_bytes(input: &'de [u8]) -> Self {
        Deserializer { input: Cursor::new(&input), le: false }
    }

    pub fn from_bytes_le(input: &'de [u8]) -> Self {
        Deserializer { input: Cursor::new(&input), le: true }
    }
}

pub fn from_bytes<'a, T>(input: &'a [u8]) -> Result<T>
where
    T: Deserialize<'a>,
{
    let mut deserializer = Deserializer::from_bytes(input);
    let t = T::deserialize(&mut deserializer)?;
    if deserializer.input.has_remaining() {
        Err(Error::TrailingBytes)
    } else {
        Ok(t)
    }
}

pub fn from_bytes_le<T>(input: &[u8]) -> Result<T>
where
    T: DeserializeOwned,
{
    let mut deserializer = Deserializer::from_bytes_le(input);
    let t = T::deserialize(&mut deserializer)?;
    if deserializer.input.has_remaining() {
        Err(Error::TrailingBytes)
    } else {
        Ok(t)
    }
}

impl<'de> Deserializer<'de> {

    fn zigzag_decode(&self, val: u64) -> i64 {
        let ret = (val >> 1) as i64;
        ret ^ -((val as i64) & 1)
    }

    fn parse_bool(&mut self) -> Result<bool> {
        if !self.input.has_remaining() {
            return Err(Error::Eof);
        }
        match self.input.get_u8() {
            0 => Ok(false),
            _ => Ok(true)
        }
    }

    // Parse a group of decimal digits as an unsigned integer of type T.
    //
    // This implementation is a bit too lenient, for example `001` is not
    // allowed in JSON. Also the various arithmetic operations can overflow and
    // panic or return bogus data. But it is good enough for example code!
    fn parse_unsigned(&mut self) -> Result<u64>
    {
        if !self.input.has_remaining() {
            return Err(Error::Eof);
        }
        let val = match self.input.get_u8() {
            0xff => {
                if self.input.remaining() < 8 {
                    return Err(Error::Eof);
                }
                if self.le { self.input.get_u64_le() } else { self.input.get_u64() }
            },
            0xfe => {
                if self.input.remaining() < 4 {
                    return Err(Error::Eof);
                }
                (if self.le { self.input.get_u32_le() } else { self.input.get_u32() }) as u64
            },
            0xfd => {
                if self.input.remaining() < 2 {
                    return Err(Error::Eof);
                }
                (if self.le { self.input.get_u16_le() } else { self.input.get_u16() }) as u64
            },
            other => other as u64
        };
        Ok(val)
    }

    fn parse_signed<T>(&mut self) -> Result<i64>
    {
        let unsigned = self.parse_unsigned()?;
        Ok(self.zigzag_decode(unsigned))
    }

    fn parse_bytes(&mut self) -> Result<Vec<u8>> {
        let len = self.parse_unsigned()? as usize;
        if self.input.remaining() < len as usize {
            return Err(Error::Eof);
        }
        let bytes = self.input.chunk()
            .get(..len)
            .map(|bytes| bytes.to_vec())
            .ok_or_else(|| Error::Eof)?;
        self.input.advance(len);
        Ok(bytes)
    }

    fn parse_string(&mut self) -> Result<String> {
        let bytes = self.parse_bytes()?;
        String::from_utf8(bytes)
            .map_err(|_| Error::Format)
    }
}

impl<'de, 'a> de::Deserializer<'de> for &'a mut Deserializer<'de> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_bool(self.parse_bool()?)
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i8(self.parse_signed::<i8>()? as i8)
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i16(self.parse_signed::<i16>()? as i16)
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i32(self.parse_signed::<i32>()? as i32)
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i64(self.parse_signed::<i64>()?)
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u8(self.parse_unsigned()? as u8)
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u16(self.parse_unsigned()? as u16)
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u32(self.parse_unsigned()? as u32)
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u64(self.parse_unsigned()?)
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        if self.input.remaining() < 4 {
            return Err(Error::Eof);
        }
        visitor.visit_f32(self.input.get_f32())
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        if self.input.remaining() < 8 {
            return Err(Error::Eof);
        }
        visitor.visit_f64(self.input.get_f64())
    }

    fn deserialize_char<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_str<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_string(self.parse_string()?)
    }

    fn deserialize_bytes<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_byte_buf(self.parse_bytes()?)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.parse_bool()? {
            false => visitor.visit_none(),
            true => visitor.visit_some(self)
        }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let size = self.parse_unsigned()? as usize;
        self.deserialize_tuple(size, visitor)
    }

    fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let value = visitor.visit_seq(TypeSeparated::new(self, len))?;
        Ok(value)
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_tuple(len, visitor)
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let size = self.parse_unsigned()? as usize;
        visitor.visit_map(TypeSeparated::new(self, size))
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let size = fields.len();
        self.deserialize_tuple(size, visitor)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let value = visitor.visit_enum(Enum::new(self))?;
        Ok(value)
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_string(visitor)
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }
}

// In order to handle commas correctly when deserializing a JSON array or map,
// we need to track whether we are on the first element or past the first
// element.
struct TypeSeparated<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
    len: usize,
    pos: usize
}

impl<'a, 'de> TypeSeparated<'a, 'de> {
    fn new(de: &'a mut Deserializer<'de>, len: usize) -> Self {
        TypeSeparated {
            de,
            len,
            pos: 0
        }
    }
}

// `SeqAccess` is provided to the `Visitor` to give it the ability to iterate
// through elements of the sequence.
impl<'de, 'a> SeqAccess<'de> for TypeSeparated<'a, 'de> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: DeserializeSeed<'de>,
    {
        if self.pos < self.len {
            self.pos += 1;
            seed.deserialize(&mut *self.de).map(Some)
        } else {
            Ok(None)
        }
        
    }
    
    fn size_hint(&self) -> Option<usize> {
        Some(self.len)
    }
}

impl<'de, 'a> MapAccess<'de> for TypeSeparated<'a, 'de> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
    where
        K: DeserializeSeed<'de>,
    {
        if self.pos < self.len {
            seed.deserialize(&mut *self.de).map(Some)
        } else {
            Ok(None)
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
    where
        V: DeserializeSeed<'de>,
    {
        self.pos += 1;
        seed.deserialize(&mut *self.de)
    }

    #[inline]
    fn size_hint(&self) -> Option<usize> {
        Some(self.len)
    }
}

struct Enum<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
}

impl<'a, 'de> Enum<'a, 'de> {
    fn new(de: &'a mut Deserializer<'de>) -> Self {
        Enum { de }
    }
}

// `EnumAccess` is provided to the `Visitor` to give it the ability to determine
// which variant of the enum is supposed to be deserialized.
//
// Note that all enum deserialization methods in Serde refer exclusively to the
// "externally tagged" enum representation.
impl<'de, 'a> EnumAccess<'de> for Enum<'a, 'de> {
    type Error = Error;
    type Variant = Self;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant)>
    where
        V: DeserializeSeed<'de>,
    {
        let val = seed.deserialize(&mut *self.de)?;
        Ok((val, self))
    }
}

impl<'de, 'a> VariantAccess<'de> for Enum<'a, 'de> {
    type Error = Error;

    fn unit_variant(self) -> Result<()> {
        Ok(())
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value>
    where
        T: DeserializeSeed<'de>,
    {
        seed.deserialize(self.de)
    }

    fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        de::Deserializer::deserialize_seq(self.de, visitor)
    }

    fn struct_variant<V>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        de::Deserializer::deserialize_map(self.de, visitor)
    }
}