use serde::ser::{SerializeSeq, SerializeTuple, SerializeTupleStruct, SerializeTupleVariant, SerializeMap, SerializeStruct, SerializeStructVariant};
use serde::Serialize;

use std::collections::BTreeMap;

use crate::Error;
use super::{Primitive, Value, Data, Number};

type Result<O> = std::result::Result<O, Error>;

pub struct Serializer;

impl serde::Serializer for Serializer {
    type Ok = Value;
    type Error = Error;
    type SerializeSeq = Sequence;
    type SerializeTuple = Tuple;
    type SerializeTupleStruct = TupleStruct;
    type SerializeTupleVariant = TupleVariant;
    type SerializeMap = Map;
    type SerializeStruct = Struct;
    type SerializeStructVariant = StructVariant;

    fn serialize_bool(self, v: bool) -> Result<Value> {Ok(Value::Primitive(Primitive::bool(v)))}
    fn serialize_i8(self, v: i8) -> Result<Value> {Ok(Value::Primitive(Primitive::Number(Number::i8(v))))}
    fn serialize_i16(self, v: i16) -> Result<Value> {Ok(Value::Primitive(Primitive::Number(Number::i16(v))))}
    fn serialize_i32(self, v: i32) -> Result<Value> {Ok(Value::Primitive(Primitive::Number(Number::i32(v))))}
    fn serialize_i64(self, v: i64) -> Result<Value> {Ok(Value::Primitive(Primitive::Number(Number::i64(v))))}
    fn serialize_u8(self, v: u8) -> Result<Value> {Ok(Value::Primitive(Primitive::Number(Number::u8(v))))}
    fn serialize_u16(self, v: u16) -> Result<Value> {Ok(Value::Primitive(Primitive::Number(Number::u16(v))))}
    fn serialize_u32(self, v: u32) -> Result<Value> {Ok(Value::Primitive(Primitive::Number(Number::u32(v))))}
    fn serialize_u64(self, v: u64) -> Result<Value> {Ok(Value::Primitive(Primitive::Number(Number::u64(v))))}
    fn serialize_f32(self, v: f32) -> Result<Value> {Ok(Value::Primitive(Primitive::Number(Number::f32(v))))}
    fn serialize_f64(self, v: f64) -> Result<Value> {Ok(Value::Primitive(Primitive::Number(Number::f64(v))))}
    fn serialize_char(self, v: char) -> Result<Value> {Ok(Value::Primitive(Primitive::char(v)))}
    fn serialize_str(self, v: &str) -> Result<Value> {Ok(Value::Primitive(Primitive::String(v.to_string())))}
    fn serialize_bytes(self, v: &[u8]) -> Result<Value> {Ok(Value::Primitive(Primitive::Bytes(v.to_vec())))}

    fn serialize_none(self) -> Result<Value> {Ok(Value::None)}
    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<Value> {Ok(Value::Some(Box::new(value.serialize(Serializer)?)))}
    fn serialize_unit(self) -> Result<Value> {Ok(Value::Primitive(Primitive::Unit))}

    fn serialize_unit_struct(self, name: &'static str) -> Result<Value> {
        Ok(Value::Struct(name.to_string(), Data::Unit))
    }
    fn serialize_unit_variant(self, name: &'static str, _: u32, variant: &'static str) -> Result<Value> {
        Ok(Value::Variant(name.to_string(), variant.to_string(), Data::Unit))
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(self, name: &'static str, value: &T) -> Result<Value> {
        Ok(Value::Struct(name.to_string(), Data::NewType(Box::new(value.serialize(Serializer)?))))
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self, name: &'static str, _: u32, variant: &'static str, value: &T
    ) -> Result<Value> {
        Ok(Value::Variant(name.to_string(), variant.to_string(), Data::NewType(Box::new(value.serialize(Serializer)?))))
    }

    fn serialize_struct(self, name: &'static str, _: usize) -> Result<Struct> {
        Ok(Struct(name.to_string(), BTreeMap::new()))
    }
    fn serialize_struct_variant(self, name: &'static str, _: u32, variant: &'static str, _: usize) -> Result<StructVariant> {
        Ok(StructVariant(name.to_string(), variant.to_string(), BTreeMap::new()))
    }

    fn serialize_tuple_struct(self, name: &'static str, _: usize) -> Result<TupleStruct> {
        Ok(TupleStruct(name.to_string(), Vec::new()))
    }
    fn serialize_tuple_variant(self, name: &'static str, _: u32, variant: &'static str, _: usize) -> Result<TupleVariant> {
        Ok(TupleVariant(name.to_string(), variant.to_string(), Vec::new()))
    }

    fn serialize_tuple(self, _: usize) -> Result<Tuple> {Ok(Tuple(Vec::new()))}
    fn serialize_seq(self, _: Option<usize>) -> Result<Sequence> {Ok(Sequence(Vec::new()))}
    fn serialize_map(self, _: Option<usize>) -> Result<Map> {Ok(Map(None, BTreeMap::new()))}
}

pub struct Map(Option<Value>, BTreeMap<Value, Value>);
impl SerializeMap for Map {
    type Ok = Value;
    type Error = Error;

    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<()> {
        self.0 = Some(key.serialize(Serializer)?);
        Ok(())
    }
    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        self.1.insert(self.0.take().unwrap(), value.serialize(Serializer)?);
        Ok(())
    }
    fn end(self) -> Result<Value> {Ok(Value::Map(self.1))}
}

pub struct Sequence(Vec<Value>);
impl SerializeSeq for Sequence {
    type Ok = Value;
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        self.0.push(value.serialize(Serializer)?);
        Ok(())
    }
    fn end(self) -> Result<Value> {Ok(Value::Vec(self.0))}
}

pub struct Tuple(Vec<Value>);
impl SerializeTuple for Tuple {
    type Ok = Value;
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        self.0.push(value.serialize(Serializer)?);
        Ok(())
    }
    fn end(self) -> Result<Value> {Ok(Value::Tuple(self.0))}
}

pub struct TupleStruct(String, Vec<Value>);
impl SerializeTupleStruct for TupleStruct {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        self.1.push(value.serialize(Serializer)?);
        Ok(())
    }
    fn end(self) -> Result<Value> {Ok(Value::Struct(self.0, Data::Tuple(self.1)))}
}

pub struct TupleVariant(String, String, Vec<Value>);
impl SerializeTupleVariant for TupleVariant {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        self.2.push(value.serialize(Serializer)?);
        Ok(())
    }
    fn end(self) -> Result<Value> {Ok(Value::Variant(self.0, self.1, Data::Tuple(self.2)))}
}

pub struct Struct(String, BTreeMap<String, Value>);
impl SerializeStruct for Struct {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, key: &'static str, value: &T) -> Result<()> {
        self.1.insert(key.to_string(), value.serialize(Serializer)?);
        Ok(())
    }
    fn end(self) -> Result<Value> {Ok(Value::Struct(self.0, Data::Struct(self.1)))}
}

pub struct StructVariant(String, String, BTreeMap<String, Value>);
impl SerializeStructVariant for StructVariant {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, key: &'static str, value: &T) -> Result<()> {
        self.2.insert(key.to_string(), value.serialize(Serializer)?);
        Ok(())
    }
    fn end(self) -> Result<Value> {Ok(Value::Variant(self.0, self.1, Data::Struct(self.2)))}
}
