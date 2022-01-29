use bytes::{BytesMut, BufMut};
use serde::{ser, Serialize};

use crate::error::{Error, Result};

pub struct Serializer {
    le: bool,
    output: BytesMut
}

pub fn to_vec_le<T>(value: &T) -> Result<Vec<u8>> 
where
    T: Serialize
{
    let mut serializer = Serializer {
        le: true,
        output: BytesMut::with_capacity(64),
    };
    value.serialize(&mut serializer)?;
    Ok(serializer.output.to_vec())
}

pub fn to_vec<T>(value: &T) -> Result<Vec<u8>>
where
    T: Serialize,
{
    let mut serializer = Serializer {
        le: false,
        // TODO capacity?
        output: BytesMut::with_capacity(1024),
    };
    value.serialize(&mut serializer)?;
    Ok(serializer.output.to_vec())
}

impl Serializer {
    fn serialize_signed(&mut self, val: i64) -> Result<()> {
        let val = self.zigzag_encode(val);
        self.serialize_unsigned(val)
    }

    fn serialize_unsigned(&mut self, val: u64) -> Result<()> {
        match val {
            n@0..=0xFC => self.output.put_u8(n as u8),
            n@0xFD..=0xFFFF => {
                self.output.put_u8(0xFD);
                if self.le {self.output.put_u16_le(n as u16)} else {self.output.put_u16(n as u16)}
            },
            n@0xFFFF01..=0xFFFFFFFF => {
                self.output.put_u8(0xFE);
                if self.le {self.output.put_u32_le(n as u32)} else {self.output.put_u32(n as u32)}
            },
            n => {
                self.output.put_u8(0xFF);
                if self.le {self.output.put_u64_le(n)} else {self.output.put_u64(n)}
            },
        }
        Ok(())
    }

    fn zigzag_encode(&self, val: i64) -> u64 {
        ((val << 1) ^ (val >> 63)) as u64
    }
}

impl<'a> ser::Serializer for &'a mut Serializer {
    // The output type produced by this `Serializer` during successful
    // serialization. Most serializers that produce text or binary output should
    // set `Ok = ()` and serialize into an `io::Write` or buffer contained
    // within the `Serializer` instance, as happens here. Serializers that build
    // in-memory data structures may be simplified by using `Ok` to propagate
    // the data structure around.
    type Ok = ();

    // The error type when some error occurs during serialization.
    type Error = Error;

    // Associated types for keeping track of additional state while serializing
    // compound data structures like sequences and maps. In this case no
    // additional state is required beyond what is already stored in the
    // Serializer struct.
    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    fn serialize_bool(self, v: bool) -> Result<()> {
        self.output.put_u8(if v {1} else {0});
        Ok(())
    }

    fn serialize_i8(self, v: i8) -> Result<()> {
        self.serialize_signed(v as i64)
    }

    fn serialize_i16(self, v: i16) -> Result<()> {
        self.serialize_signed(v as i64)
    }

    fn serialize_i32(self, v: i32) -> Result<()> {
        self.serialize_signed(v as i64)
    }

    fn serialize_i64(self, v: i64) -> Result<()> {
        self.serialize_signed(v as i64)
    }

    fn serialize_u8(self, v: u8) -> Result<()> {
        self.serialize_unsigned(v as u64)
    }

    fn serialize_u16(self, v: u16) -> Result<()> {
        self.serialize_unsigned(v as u64)
    }

    fn serialize_u32(self, v: u32) -> Result<()> {
        self.serialize_unsigned(v as u64)
    }

    fn serialize_u64(self, v: u64) -> Result<()> {
        self.serialize_unsigned(v as u64)
    }

    fn serialize_f32(self, v: f32) -> Result<()> {
        if self.le { self.output.put_f32_le(v) } else { self.output.put_f32(v) };
        Ok(())
    }

    fn serialize_f64(self, v: f64) -> Result<()> {
        if self.le { self.output.put_f64_le(v) } else { self.output.put_f64(v) };
        Ok(())
    }

    fn serialize_char(self, v: char) -> Result<()> {
        self.serialize_str(&v.to_string())
    }

    fn serialize_str(self, v: &str) -> Result<()> {
        let bytes = v.as_bytes();
        self.serialize_bytes(bytes)?;
        Ok(())
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<()> {
        self.serialize_u64(v.len() as u64)?;
        self.output.put(v);
        Ok(())
    }

    fn serialize_none(self) -> Result<()> {
        self.serialize_bool(false)
    }

    fn serialize_some<T>(self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.serialize_bool(true)?;
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<()> {
        Ok(())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        self.serialize_unit()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<()> {
        Ok(())
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        value: &T,
    ) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut *self)?;
        Ok(())
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq> {
        if let Some(len) = len {
            self.serialize_u64(len as u64)?;
        } else {
            unimplemented!("only support known size of seq")
        }
        Ok(self)
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        Ok(self)
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap> {
        self.serialize_seq(len)
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct> {
        Ok(self)
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        Ok(self)
    }
}


impl<'a> ser::SerializeSeq for &'a mut Serializer {
    // Must match the `Ok` type of the serializer.
    type Ok = ();
    // Must match the `Error` type of the serializer.
    type Error = Error;

    // Serialize a single element of the sequence.
    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    // Close the sequence.
    fn end(self) -> Result<()> {
        Ok(())
    }
}

// Same thing but for tuples.
impl<'a> ser::SerializeTuple for &'a mut Serializer {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

// Same thing but for tuple structs.
impl<'a> ser::SerializeTupleStruct for &'a mut Serializer {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl<'a> ser::SerializeTupleVariant for &'a mut Serializer {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl<'a> ser::SerializeMap for &'a mut Serializer {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T>(&mut self, key: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        key.serialize(&mut **self)
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl<'a> ser::SerializeStruct for &'a mut Serializer {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, _key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl<'a> ser::SerializeStructVariant for &'a mut Serializer {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, _key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}
