use serde::de::{Visitor, SeqAccess, MapAccess, DeserializeSeed, Unexpected, Error as _, VariantAccess, EnumAccess};
use serde::{forward_to_deserialize_any};

use std::collections::BTreeMap;

use crate::value::{Primitive, Value, Number, Data};
use crate::Error;

type Result<V> = std::result::Result<V, Error>;

impl<'de> serde::Deserializer<'de> for Value {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self {
            Value::Primitive(field) => match field {
                Primitive::Unit => visitor.visit_unit(),
                Primitive::bool(b) => visitor.visit_bool(b),
                Primitive::Number(number) => match number {
                    Number::i8(i) => visitor.visit_i8(i),
                    Number::i16(i) => visitor.visit_i16(i),
                    Number::i32(i) => visitor.visit_i32(i),
                    Number::i64(i) => visitor.visit_i64(i),
                    Number::i128(i) => visitor.visit_i128(i),
                    Number::u8(i) => visitor.visit_u8(i),
                    Number::u16(i) => visitor.visit_u16(i),
                    Number::u32(i) => visitor.visit_u32(i),
                    Number::u64(i) => visitor.visit_u64(i),
                    Number::u128(i) => visitor.visit_u128(i),
                    Number::f32(i) => visitor.visit_f32(i),
                    Number::f64(i) => visitor.visit_f64(i),
                }
                Primitive::char(c) => visitor.visit_char(c),
                Primitive::String(s) => visitor.visit_string(s),
                Primitive::Bytes(b) => visitor.visit_byte_buf(b)
            },
            Value::Variant(_name, variant, data) => visitor.visit_enum(Variant(variant, data)),
            Value::Struct(_name, data) => match data {
                Data::Unit => visitor.visit_map(Struct(BTreeMap::new(), None)),
                Data::Tuple(objects) => visitor.visit_seq(Seq::new(objects)),
                Data::NewType(i) => visitor.visit_newtype_struct(*i),
                Data::Struct(fields) => visitor.visit_map(Struct(fields, None))
            },
            Value::Tuple(objects) => visitor.visit_seq(Seq::new(objects)),
            Value::Vec(objects) => visitor.visit_seq(Seq::new(objects)),
            Value::Map(objects) => visitor.visit_map(Map(objects, None)),
            Value::Some(b) => visitor.visit_some(*b),
            Value::None => visitor.visit_none(),
        }
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        self.deserialize_any(visitor)
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct identifier ignored_any
    }
}

pub struct Map(BTreeMap<Value, Value>, Option<Value>);
impl<'de> MapAccess<'de> for Map {
    type Error = Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(&mut self, seed: K) -> Result<Option<K::Value>> {
        self.0.pop_first().map(|(k, v)| {self.1 = Some(v); seed.deserialize(k)}).transpose()
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value> {
        seed.deserialize(self.1.take().unwrap())
    }
}

pub struct Struct(BTreeMap<String, Value>, Option<Value>);
impl<'de> MapAccess<'de> for Struct {
    type Error = Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(&mut self, seed: K) -> Result<Option<K::Value>> {
        self.0.pop_first().map(|(k, v)| {self.1 = Some(v); seed.deserialize(Value::Primitive(Primitive::String(k)))}).transpose()
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value> {
        seed.deserialize(self.1.take().unwrap())
    }
}

pub struct Seq(Vec<Value>);
impl Seq {
    pub fn new(objects: Vec<Value>) -> Self {Seq(objects.into_iter().rev().collect())}
}
impl<'de> SeqAccess<'de> for Seq {
    type Error = Error;

    fn next_element_seed<T: DeserializeSeed<'de>>(&mut self, seed: T) -> Result<Option<T::Value>> {
        self.0.pop().map(|o| seed.deserialize(o)).transpose()
    }
}

impl<'de> VariantAccess<'de> for Data {
    type Error = Error;

    fn unit_variant(self) -> Result<()> {
        match self {
            Data::Unit => Ok(()),
            Data::NewType(_) => Err(Error::invalid_type(Unexpected::NewtypeVariant, &"unit variant")),
            Data::Tuple(_) => Err(Error::invalid_type(Unexpected::TupleVariant, &"unit variant")),
            Data::Struct(_) => Err(Error::invalid_type(Unexpected::StructVariant, &"unit variant"))
        }
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value> {
        match self {
            Data::Unit => Err(Error::invalid_type(Unexpected::UnitVariant, &"newtype variant")),
            Data::NewType(i) => seed.deserialize(*i),
            Data::Tuple(_) => Err(Error::invalid_type(Unexpected::TupleVariant, &"newtype variant")),
            Data::Struct(_) => Err(Error::invalid_type(Unexpected::StructVariant, &"newtype variant"))
        }
    }

    fn tuple_variant<V: Visitor<'de>>(self, _len: usize, visitor: V) -> Result<V::Value> {
        match self {
            Data::Unit => Err(Error::invalid_type(Unexpected::UnitVariant, &"tuple variant")),
            Data::NewType(_) => Err(Error::invalid_type(Unexpected::NewtypeVariant, &"tuple variant")),
            Data::Tuple(objects) => visitor.visit_seq(Seq::new(objects)),
            Data::Struct(_) => Err(Error::invalid_type(Unexpected::StructVariant, &"tuple variant"))
        }
    }

    fn struct_variant<V: Visitor<'de>>(self, _fields: &'static [&'static str], visitor: V) -> Result<V::Value> {
        match self {
            Data::Unit => Err(Error::invalid_type(Unexpected::UnitVariant, &"struct variant")),
            Data::NewType(_) => Err(Error::invalid_type(Unexpected::NewtypeVariant, &"struct variant")),
            Data::Tuple(_) => Err(Error::invalid_type(Unexpected::TupleVariant, &"struct variant")),
            Data::Struct(fields) => visitor.visit_map(Struct(fields, None))
        }
    }
}

pub struct Variant(String, Data);
impl<'de> EnumAccess<'de> for Variant {
    type Error = Error;
    type Variant = Data;

    fn variant_seed<V: DeserializeSeed<'de>>(self, seed: V) -> Result<(V::Value, Data)> {
        seed.deserialize(Value::Primitive(Primitive::String(self.0))).map(|v| (v, self.1))
    }
}
