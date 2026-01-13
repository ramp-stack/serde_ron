use serde::{Serialize, Deserialize};
mod ser;
pub use ser::Serializer;
mod de;
//pub use de::Deserializer;

use std::collections::BTreeMap;
use std::hash::{Hasher, Hash};
use std::cmp::Ordering;

#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum Number {
    i8(i8), i16(i16), i32(i32), i64(i64), i128(i128),
    u8(u8), u16(u16), u32(u32), u64(u64), u128(u128),
    f32(f32), f64(f64)
}
impl Number {
    fn break_down(self) -> (bool, u128, f32) {match self {
        Self::u8(b) => (false, b as u128, 0.0),
        Self::u16(b) => (false, b as u128, 0.0),
        Self::u32(b) => (false, b as u128, 0.0),
        Self::u64(b) => (false, b as u128, 0.0),
        Self::u128(b) => (false, b, 0.0),
        Self::i8(i) => (i < 0, i.unsigned_abs() as u128, 0.0),
        Self::i16(i) => (i < 0, i.unsigned_abs() as u128, 0.0),
        Self::i32(i) => (i < 0, i.unsigned_abs() as u128, 0.0),
        Self::i64(i) => (i < 0, i.unsigned_abs() as u128, 0.0),
        Self::i128(i) => (i < 0, i.unsigned_abs(), 0.0),
        Self::f32(i) => (i < 0.0, i.abs().floor() as u128, i.abs() % 0.0),
        Self::f64(i) => (i < 0.0, i.abs().floor() as u128, (i.abs() % 0.0) as f32)
    }}
}
impl PartialEq for Number {
    fn eq(&self, other: &Self) -> bool {self.break_down() == other.break_down()}
}
impl Eq for Number {}

impl PartialOrd for Number {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {Some(self.cmp(other))}
}
impl Ord for Number {
    fn cmp(&self, other: &Self) -> Ordering {match (self.break_down(), other.break_down()) {
        ((a_neg, _, _), (b_neg, _, _)) if a_neg != b_neg => (!a_neg).cmp(&(!b_neg)),
        //Both numbers are positive or negitive
        ((neg, a_num, _), (_, b_num, _)) if a_num != b_num && !neg => a_num.cmp(&b_num),
        ((_, a_num, _), (_, b_num, _)) if a_num != b_num => b_num.cmp(&a_num),
        //Both numbers are equal except the decimal
        ((neg, _, a_dec), (_, _, b_dec)) if !neg => a_dec.total_cmp(&b_dec),
        ((_, _, a_dec), (_, _, b_dec)) => b_dec.total_cmp(&a_dec)
    }}
}
impl Hash for Number {
    fn hash<H: Hasher>(&self, state: &mut H) {match self {
        Self::u8(i) => i.hash(state),
        Self::u16(i) => i.hash(state),
        Self::u32(i) => i.hash(state),
        Self::u64(i) => i.hash(state),
        Self::u128(i) => i.hash(state),
        Self::i8(i) => i.hash(state),
        Self::i16(i) => i.hash(state),
        Self::i32(i) => i.hash(state),
        Self::i64(i) => i.hash(state),
        Self::i128(i) => i.hash(state),
        Self::f32(i) => i.to_bits().hash(state),
        Self::f64(i) => i.to_bits().hash(state),
    }}
}

impl std::fmt::Display for Number {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {match self {
        Self::u8(i) => write!(f, "{}", i),
        Self::u16(i) => write!(f, "{}", i),
        Self::u32(i) => write!(f, "{}", i),
        Self::u64(i) => write!(f, "{}", i),
        Self::u128(i) => write!(f, "{}", i),
        Self::i8(i) => write!(f, "{}", i),
        Self::i16(i) => write!(f, "{}", i),
        Self::i32(i) => write!(f, "{}", i),
        Self::i64(i) => write!(f, "{}", i),
        Self::i128(i) => write!(f, "{}", i),
        Self::f32(i) => write!(f, "{}", i),
        Self::f64(i) => write!(f, "{}", i),
    }}
}

#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum Primitive {
    Unit,
    bool(bool),
    Number(Number),
    char(char),
    String(String),
    Bytes(Vec<u8>)
}
impl Primitive {
    fn as_number(&self) -> Option<Number> {match self {
        Self::Unit => None,
        Self::bool(b) => Some(Number::u8(*b as u8)),
        Self::Number(n) => Some(*n),
        Self::char(c) => c.to_digit(10).map(|d| Number::u8(d as u8)),
        Self::String(s) => s.parse::<u128>().map(Number::u128).or_else(|_| s.parse::<i128>().map(Number::i128)).ok(),
        Self::Bytes(b) if b.len() < 16 => {
            let mut arr = [0u8; 16];
            arr[..b.len()].copy_from_slice(b);
            Some(Number::u128(u128::from_le_bytes(arr)))
        }
        Self::Bytes(_) => Some(Number::u128(u128::MAX))
    }}
}

impl PartialOrd for Primitive {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Primitive {
    fn cmp(&self, other: &Self) -> Ordering {
        let a = self.as_number();
        let b = other.as_number();
        let ord = a.and_then(|a| b.map(|b| a.cmp(&b)));
        match (self, other) {
            (Self::Bytes(a), Self::Bytes(b)) => a.cmp(b),
            (a, b) if a == b => Ordering::Equal,
            (Self::Unit, _) => Ordering::Less,
            (_, Self::Unit) => Ordering::Greater,
            _ if ord.is_some() => ord.unwrap(),
            (a, b) => a.to_string().cmp(&b.to_string())
        }
    }
}

impl std::fmt::Display for Primitive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {match self {
        Self::Unit => write!(f, "()"),
        Self::bool(b) => write!(f, "{}", b),
        Self::Number(n) => write!(f, "{}", n),
        Self::char(c) => write!(f, "{}", c),
        Self::String(s) => write!(f, "{}", s),
        Self::Bytes(b) => write!(f, "{}", std::str::from_utf8(b).map(|s| s.to_string()).unwrap_or_else(|_| format!("{:02x?}", b)))
    }}
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Data {
    Struct(BTreeMap<String, Value>),
    NewType(Box<Value>),
    Tuple(Vec<Value>),
    Unit
}

///Value::Vec(vec![Value::Primitive(Value::Primitive(Number::u8(10)))]) != Value::Primitive(Primitive::Bytes(vec![10]))
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Value {
    Variant(String, String, Data),
    Struct(String, Data),

    Tuple(Vec<Self>),
    Vec(Vec<Self>),
    Map(BTreeMap<Self, Self>),
    Primitive(Primitive),

    Some(Box<Self>), None//Variant("Option", "None", Data::Unit)
}

//  #[allow(non_camel_case_types)]
//  #[derive(Serialize, Deserilaize, Debug, Clone, PartialEq)]
//  pub enum ValueRef<'a> {
//      bool(&'a bool), i8(&'a i8), i16(&'a i16), i32(&'a i32), i64(&'a i64), i128(&'a i128), u8(&'a u8), u16(&'a u16), u32(&'a u32),
//      u64(&'a u64), u128(&'a u128), f32(&'a f32), f64(&'a f64), char(&'a char), String(&'a str), Bytes(&'a[u8]), Unit
//  }

//  #[derive(Debug, Clone, PartialEq)]
//  pub enum DataRef<'a> {
//      NewType(Box<ObjectRef<'a>>),
//      Tuple(Vec<ObjectRef<'a>>),
//      Struct(BTreeMap<String, ObjectRef<'a>>),
//      Unit
//  }

//  #[derive(Debug, Clone, PartialEq)]
//  pub enum ObjectRef<'a> {
//      Variant(String, String, DataRef<'a>),
//      Struct(String, DataRef<'a>),

//      Tuple(Vec<Self>),
//      Vec(Vec<Self>),
//      Map(BTreeMap<Self, Self>),
//      Field(ValueRef<'a>),

//      Some(Box<Self>), None//Variant("Option", "None", Data::Unit)
//  }


//  #[allow(non_camel_case_types)]
//  #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Copy)]
//  pub enum Type {
//      bool, i8, i16, i32, i64, i128, u8, u16, u32, u64, u128, f32, f64, char, String, Bytes, Unit
//  }

//  #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
//  pub enum DataType {
//      NewType(Box<ObjectType>),
//      Tuple(usize, Vec<ObjectType>),
//      Struct(BTreeMap<String, ObjectType>),
//      Unit
//  }

//  #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
//  pub enum ObjectType {
//      Variant(String, String, DataType),
//      Struct(String, DataType),

//      Tuple(Vec<Self>),
//      Vec(Box<Self>),
//      Map(Box<Self>, Box<Self>),
//      Field(Type),

//      Some(Box<Self>),//Variant("Option", "Some", Data::NewType(Box<Self>))
//      None//Variant("Option", "None", Data::Unit)
//  }
