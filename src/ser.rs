use serde::ser::{SerializeSeq, SerializeTuple, SerializeTupleStruct, SerializeTupleVariant, SerializeMap, SerializeStruct, SerializeStructVariant};
use serde::Serialize;
use rusqlite::{Connection};

use std::collections::{BTreeMap, BTreeSet};
use std::path::{PathBuf, Path};
use std::str::FromStr;

use crate::table::{TableAccess, Value, Structure, Primitive, Data};
use crate::{Collection, Error};

type Result = std::result::Result<(), Error>;
type ResultCompound<'a, 'b> = std::result::Result<Compound<'a, 'b>, Error>;

#[derive(Debug)]
pub struct Serializer<'b> {
    access: &'b mut TableAccess<'b>,
    offset: PathBuf,
    some: bool 
}

impl<'a> Serializer<'a> {
    pub fn new<P: AsRef<Path>>(access: &'a mut TableAccess<'a>, offset: P) -> Self {
        let mut offset = offset.as_ref().to_path_buf();
        if offset == PathBuf::new() {offset = PathBuf::from("/");}
        Serializer{access, offset, some: false}
    }

    pub fn serialize_primitive(&mut self, value: Value) -> Result {
        if self.offset.components().count() < 2 {panic!("Cannot serialize primitive")}
        self.access.insert(&self.offset, Data::Primitive(value))?;
        Ok(())
    }
}

impl<'a, 'b> serde::Serializer for &'a mut Serializer<'b> {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = Compound<'a, 'b>;
    type SerializeTuple = Compound<'a, 'b>;
    type SerializeTupleStruct = Compound<'a, 'b>;
    type SerializeTupleVariant = Compound<'a, 'b>;
    type SerializeMap = Compound<'a, 'b>;
    type SerializeStruct = Compound<'a, 'b>;
    type SerializeStructVariant = Compound<'a, 'b>;

    fn serialize_bool(self, v: bool) -> Result {self.serialize_primitive(Value::bool(v))}
    fn serialize_i8(self, v: i8) -> Result {self.serialize_primitive(Value::i8(v))}
    fn serialize_i16(self, v: i16) -> Result {self.serialize_primitive(Value::i16(v))}
    fn serialize_i32(self, v: i32) -> Result {self.serialize_primitive(Value::i32(v))}
    fn serialize_i64(self, v: i64) -> Result {self.serialize_primitive(Value::i64(v))}
    fn serialize_u8(self, v: u8) -> Result {self.serialize_primitive(Value::u8(v))}
    fn serialize_u16(self, v: u16) -> Result {self.serialize_primitive(Value::u16(v))}
    fn serialize_u32(self, v: u32) -> Result {self.serialize_primitive(Value::u32(v))}
    fn serialize_u64(self, v: u64) -> Result {self.serialize_primitive(Value::u64(v))}
    fn serialize_f32(self, v: f32) -> Result {self.serialize_primitive(Value::f32(v))}
    fn serialize_f64(self, v: f64) -> Result {self.serialize_primitive(Value::f64(v))}
    fn serialize_char(self, v: char) -> Result {self.serialize_primitive(Value::char(v))}
    fn serialize_str(self, v: &str) -> Result {self.serialize_primitive(Value::String(v.to_string()))}
    fn serialize_bytes(self, v: &[u8]) -> Result {self.serialize_primitive(Value::Bytes(v.to_vec()))}

    fn serialize_none(self) -> Result {
        self.access.delete(&self.offset)?;
        Ok(())
    }

    //Nested Options will either output None or Some(Some(x)) never Some(None)
    fn serialize_some<T: Serialize + ?Sized>(mut self, value: &T) -> Result {self.some = true; value.serialize(self)}
    //Option<()> will always deserialize as None
    fn serialize_unit(self) -> Result {self.serialize_primitive(Value::Null)}

    fn serialize_unit_struct(self, name: &'static str) -> Result {
        Compound::new(self, name.to_string(), Structure::Struct(BTreeMap::new())).write()
    }
    fn serialize_unit_variant(self, name: &'static str, _: u32, variant: &'static str) -> Result {
        Compound::new(self, name.to_string(), Structure::Variant(variant.to_string(), Box::new(Structure::Struct(BTreeMap::new())))).write()
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(self, name: &'static str, value: &T) -> Result {
        let mut compound = Compound::new(self, name.to_string(), Structure::Null);
        value.serialize(&mut compound)?;
        compound.write()
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(self, name: &'static str, _: u32, variant: &'static str, value: &T) -> Result {
        let mut compound = Compound::new(self, name.to_string(), Structure::Variant(variant.to_string(), Box::new(Structure::Null)));
        value.serialize(&mut compound)?;
        compound.write()
    }

    fn serialize_struct(self, name: &'static str, _: usize) -> ResultCompound<'a, 'b> {
        Ok(Compound::new(self, name.to_string(), Structure::Struct(BTreeMap::new())))
    }
    fn serialize_struct_variant(self, name: &'static str, _: u32, variant: &'static str, _: usize) -> ResultCompound<'a, 'b> {
        Ok(Compound::new(self, name.to_string(), Structure::Variant(variant.to_string(), Box::new(Structure::Struct(BTreeMap::new())))))
    }

    fn serialize_tuple_struct(self, name: &'static str, _: usize) -> ResultCompound<'a, 'b> {
        Ok(Compound::new(self, name.to_string(), Structure::Struct(BTreeMap::new())))
    }
    fn serialize_tuple_variant(self, name: &'static str, _: u32, variant: &'static str, _: usize) -> ResultCompound<'a, 'b> {
        Ok(Compound::new(self, name.to_string(), Structure::Variant(variant.to_string(), Box::new(Structure::Struct(BTreeMap::new())))))
    }

    ///Error on collections not having a new type
    fn serialize_tuple(self, len: usize) -> ResultCompound<'a, 'b> {Err(Error::NewType(Collection::Tuple(len)))}
    fn serialize_seq(self, _: Option<usize>) -> ResultCompound<'a, 'b> {Err(Error::NewType(Collection::Vec))}
    fn serialize_map(self, _: Option<usize>) -> ResultCompound<'a, 'b> {Err(Error::NewType(Collection::Map))}
}

#[derive(Debug)]
pub struct Compound<'a, 'b> {
    serializer: &'a mut Serializer<'b>,
    name: String,
    some: bool,
    structure: Structure,
    key: Option<String>,
    idx: u32
}
impl<'a, 'b> Compound<'a, 'b> {
    pub fn new(serializer: &'a mut Serializer<'b>, name: String, structure: Structure) -> Self {
        Compound{
            name,
            some: serializer.some,
            serializer,
            structure,
            key: None,
            idx: 0
        }
    }

    pub fn serialize_key(&mut self, key: String) -> Result {
        self.key = Some(key);
        Ok(())
    }

    pub fn serialize_value(&mut self, value: Value) -> Result {
        let key = self.key.take().expect("Keyless Value");
        match self.structure.latest() {
            Structure::Struct(columns) | Structure::Map(columns) => {columns.insert(key, value);},
            _ => panic!("Cannot add value to: {:?}", self.structure)
        }
        Ok(())
    }

    pub fn serialize_element<V: Serialize + ?Sized>(&mut self, value: &V) -> Result {
        self.key = Some(format!("{}", self.idx));
        self.idx += 1;
        value.serialize(&mut *self)?;
        Ok(())
    }

    pub fn serialize_new(&mut self, name: String, structure: Structure) -> ResultCompound<'a, 'b> {
        let key = self.key.take().expect("Keyless Value");
        Ok(Compound {
            serializer: self.serializer,
            name,
            some: false,
            structure,
            key: None,
            idx: 0
        })

      //Ok(match (&self.ty, ty) {
      //    (Type::NewType(name), ty) => Compound::new(self.sqlite, self.offset.clone(), match ty {
      //        Type::NewType(_) => Type::NewType(name),
      //        Type::Struct(_) => Type::Struct(name),
      //        Type::Tuple(_, len) => Type::Tuple(name, len),
      //        Type::Map(_) => Type::Map(name),
      //        Type::Vec(_) => Type::Vec(name),
      //    }),
      //    (old, ty) => {
      //        match &ty {
      //            Type::Tuple(_, len) | Type::Map(_) | Type::Vec(_) => {Err(Error::MustNewType)}
      //            _ => {}
      //        }
      //        Compound::new(self.sqlite, self.offset.join(key), ty)
      //    }
      //})
    }

    pub fn serialize_collection(&mut self, collection: Collection) -> ResultCompound<'a, 'b> {
        todo!()
    }

    pub fn delete(&mut self) -> Result {
        if self.key.is_some() {
            //Value?
        }
        Ok(())
    }

    pub fn write(mut self) -> Result {
        todo!()
      //match self.ty {
      //    Type::Struct(name) => {
      //        let columns: BTreeMap<String, rusqlite::types::Type> = self.columns.iter().map(|(n, (t, _))|
      //            (t.field(&n), t.sqlite_type())
      //        ).collect();
      //        let table = match Table::by_name(self.sqlite, name.clone())? {
      //            Some(mut table) => {
      //                columns.into_iter().filter(|(k, _)| table.columns().contains_key(k)).collect::<Vec<_>>().into_iter().try_for_each(|(n, st)|
      //                    table.add_column(n, st)
      //                )?;
      //                table
      //            },
      //            None => Table::new(self.sqlite, name.clone(), columns)?
      //        };
      //        Self::insert(&table, &self.offset, self.columns)?;
      //        let mut compound = Compound::new(self.sqlite, self.offset, Type::Map);
      //        compound.map_ids = self.map_ids;
      //        compound.write()?;
      //    },
      //    Type::NewType(_) => {
      //        if !self.columns.is_empty() {
      //            let columns = Primitive::each().into_iter().map(|p|
      //                (p.field(VALUE), p.sqlite_type())
      //            ).collect();
      //            let table = Table::new(self.sqlite, NEW_TYPE_PRIMITIVE.to_string(), columns)?;
      //            let (t, v) = self.columns.pop_first().unwrap().1;
      //            Self::insert(&table, &self.offset, BTreeMap::from([(VALUE.to_string(), (t, v))]))?;
      //        }
      //    },
      //    Type::Map => {
      //        let mut columns = Primitive::each().into_iter().map(|p|
      //            (p.field(VALUE), p.sqlite_type())
      //        ).collect::<BTreeMap<String, rusqlite::types::Type>>();
      //        let mut table = Table::new(self.sqlite, "Map".to_string(), columns)?;
      //        for (key, (ty, value)) in self.columns {
      //            let c = BTreeMap::from([(VALUE.to_string(), (ty, value))]);
      //            Self::insert(&table, &self.offset.join(key.split(FS).collect::<Vec<_>>()[0]), c)?;
      //        }
      //        for key in self.map_ids {
      //            Self::insert(&table, &self.offset.join(key), BTreeMap::new())?;
      //        }
      //    }, 
      //    Type::Vec => {
      //        let mut columns = Primitive::each().into_iter().map(|p|
      //            (p.field(VALUE), p.sqlite_type())
      //        ).collect::<BTreeMap<String, rusqlite::types::Type>>();
      //        columns.insert(Primitive::u64.field(INDEX), Primitive::u64.sqlite_type());
      //        let mut table = Table::new(self.sqlite, "Vec".to_string(), columns)?;
      //        for (key, (ty, value)) in self.columns {
      //            let c = BTreeMap::from([
      //                (INDEX.to_string(), (Primitive::u64, Value::Integer(key.parse().unwrap()))),
      //                (VALUE.to_string(), (ty, value))
      //            ]);
      //            Self::insert(&table, &self.offset.join(key), c)?;
      //        }
      //        for key in self.map_ids {
      //            let c = BTreeMap::from([
      //                (INDEX.to_string(), (Primitive::u64, Value::Integer(key.parse().unwrap()))),
      //            ]);
      //            Self::insert(&table, &self.offset.join(key), c)?;
      //        }
      //    },
      //    Type::Tuple(len) => {
      //        let columns = (0..len).flat_map(|i| Primitive::each().into_iter().map(|p|
      //            (p.field(&i.to_string()), p.sqlite_type())
      //        ).collect::<BTreeMap<_, _>>()).collect();
      //        let table = Table::new(self.sqlite, format!("({})", len), columns)?;
      //        Self::insert(&table, &self.offset, self.columns)?;
      //    }
      //}
    }
}

impl<'a, 'b> serde::Serializer for &mut Compound<'a, 'b> {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = Compound<'a, 'b>;
    type SerializeTuple = Compound<'a, 'b>;
    type SerializeTupleStruct = Compound<'a, 'b>;
    type SerializeTupleVariant = Compound<'a, 'b>;
    type SerializeMap = Compound<'a, 'b>;
    type SerializeStruct = Compound<'a, 'b>;
    type SerializeStructVariant = Compound<'a, 'b>;

    fn serialize_bool(self, v: bool) -> Result {self.serialize_value(Value::bool(v))}
    fn serialize_i8(self, v: i8) -> Result {self.serialize_value(Value::i8(v))}
    fn serialize_i16(self, v: i16) -> Result {self.serialize_value(Value::i16(v))}
    fn serialize_i32(self, v: i32) -> Result {self.serialize_value(Value::i32(v))}
    fn serialize_i64(self, v: i64) -> Result {self.serialize_value(Value::i64(v))}
    fn serialize_u8(self, v: u8) -> Result {self.serialize_value(Value::u8(v))}
    fn serialize_u16(self, v: u16) -> Result {self.serialize_value(Value::u16(v))}
    fn serialize_u32(self, v: u32) -> Result {self.serialize_value(Value::u32(v))}
    fn serialize_u64(self, v: u64) -> Result {self.serialize_value(Value::u64(v))}
    fn serialize_f32(self, v: f32) -> Result {self.serialize_value(Value::f32(v))}
    fn serialize_f64(self, v: f64) -> Result {self.serialize_value(Value::f64(v))}
    fn serialize_char(self, v: char) -> Result {self.serialize_value(Value::char(v))}
    fn serialize_str(self, v: &str) -> Result {self.serialize_value(Value::String(v.to_string()))}
    fn serialize_bytes(self, v: &[u8]) -> Result {self.serialize_value(Value::Bytes(v.to_vec()))}

    fn serialize_none(self) -> Result {self.delete()}
    fn serialize_some<T: Serialize + ?Sized>(mut self, value: &T) -> Result {self.some = true; value.serialize(self)}
    fn serialize_unit(self) -> Result {self.serialize_value(Value::Null)}

    fn serialize_unit_struct(self, name: &'static str) -> Result {
        self.serialize_new(name.to_string(), Structure::Struct(BTreeMap::new()))?.write()
    }
    fn serialize_unit_variant(self, name: &'static str, _: u32, variant: &'static str) -> Result {
        self.serialize_new(name.to_string(), Structure::Variant(variant.to_string(), Box::new(Structure::Struct(BTreeMap::new()))))?.write()
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(self, name: &'static str, value: &T) -> Result {
        let mut compound = self.serialize_new(name.to_string(), Structure::Null)?;
        value.serialize(&mut compound)?;
        compound.write()
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self, name: &'static str, _: u32, variant: &'static str, value: &T
    ) -> Result {
        let mut compound = self.serialize_new(name.to_string(), Structure::Variant(variant.to_string(), Box::new(Structure::Null)))?;
        value.serialize(&mut compound)?;
        compound.write()
    }

    fn serialize_struct(self, name: &'static str, _: usize) -> ResultCompound<'a, 'b> {
        self.serialize_new(name.to_string(), Structure::Struct(BTreeMap::new()))
    }
    fn serialize_struct_variant(self, name: &'static str, _: u32, variant: &'static str, _: usize) -> ResultCompound<'a, 'b> {
        self.serialize_new(name.to_string(), Structure::Variant(variant.to_string(), Box::new(Structure::Struct(BTreeMap::new()))))
    }

    fn serialize_tuple_struct(self, name: &'static str, _: usize) -> ResultCompound<'a, 'b> {
        self.serialize_new(name.to_string(), Structure::Struct(BTreeMap::new()))
    }
    fn serialize_tuple_variant(self, name: &'static str, _: u32, variant: &'static str, _: usize) -> ResultCompound<'a, 'b> {
        self.serialize_new(name.to_string(), Structure::Variant(variant.to_string(), Box::new(Structure::Struct(BTreeMap::new()))))
    }

    fn serialize_tuple(self, len: usize) -> ResultCompound<'a, 'b> {self.serialize_collection(Collection::Tuple(len))}
    fn serialize_seq(self, _: Option<usize>) -> ResultCompound<'a, 'b> {self.serialize_collection(Collection::Vec)}
    fn serialize_map(self, _: Option<usize>) -> ResultCompound<'a, 'b> {self.serialize_collection(Collection::Map)}
}

impl<'a, 'b> SerializeSeq for Compound<'a, 'b> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result {
        self.serialize_element(value)
    }
    fn end(self) -> Result {self.write()}
}

impl<'a, 'b> SerializeTuple for Compound<'a, 'b> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result {
        Compound::serialize_element(self, value)
    }
    fn end(self) -> Result {self.write()}
}

impl<'a, 'b> SerializeTupleStruct for Compound<'a, 'b> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result {
        Compound::serialize_element(self, value)
    }
    fn end(self) -> Result {self.write()}
}

impl<'a, 'b> SerializeTupleVariant for Compound<'a, 'b> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result {
        Compound::serialize_element(self, value)
    }
    fn end(self) -> Result {self.write()}
}

impl<'a, 'b> SerializeMap for Compound<'a, 'b> {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result {
        let key = match serde_json::to_value(key)? {
            serde_json::Value::String(key) => Ok(key),
            _ => Err(Error::StringlessMapKey)
        }?;
        self.serialize_key(key)
    }
    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result {
        value.serialize(&mut *self)?;
        Ok(())
    }
    fn end(self) -> Result {self.write()}
}

impl<'a, 'b> SerializeStruct for Compound<'a, 'b> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, key: &'static str, value: &T) -> Result {
        self.serialize_key(key.to_string())?;
        value.serialize(self)
    }
    fn end(self) -> Result {self.write()}
}

impl<'a, 'b> SerializeStructVariant for Compound<'a, 'b> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, key: &'static str, value: &T) -> Result {
        self.serialize_key(key.to_string())?;
        value.serialize(self)
    }
    fn end(self) -> Result {self.write()}
}
