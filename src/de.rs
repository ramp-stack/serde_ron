use serde::de::{Visitor, SeqAccess, MapAccess, DeserializeSeed, Unexpected, Error as _, VariantAccess, EnumAccess};
use serde::Deserialize;
use rusqlite::{Connection, types::Value};

use std::collections::{BTreeMap, BTreeSet};
use std::path::{PathBuf, Path};
use std::str::FromStr;

use crate::table::Table;
use crate::{Error, Primitive, NEW_TYPE_PRIMITIVE, VALUE, VS, FS};

type Result<V> = std::result::Result<V, Error>;
type Columns = BTreeMap<String, (Primitive, Value)>;

fn deserialize_type(sqlite: &Connection, offset: &PathBuf) -> Result<Option<(Type, Columns)>> {
    println!("deserialize_type");
    Table::try_find_map(sqlite, |table| {
        if table.name() == "Map" || table.name() == "Vec" {return Ok(None);}
        Ok(table.get(offset, None)?.map(|c| (table.name().to_string(), c)))
    })?.map(|(name, c)| {
        let columns = Primitive::parse_many(c)?;
        let mut ty = Type::from_str(&name, columns.keys().cloned().collect())?;
        match &mut ty {
            Type::Struct(_, fields) => {
                if let Some(table) = Table::by_name(sqlite, "Map".to_string())? {
                    fields.extend(table.get_children(offset, Some(BTreeSet::new()))?.into_keys());
                }
            },
            _ => {}
        }
        Ok::<_, Error>((ty, columns))
    }).or_else(|| deserialize_map(sqlite, offset).map(|(k, c)| (!k.is_empty()).then_some((Type::Map(k), c))).transpose()).transpose()
}

fn deserialize_map(sqlite: &Connection, offset: &PathBuf) -> Result<(BTreeSet<String>, Columns)> {
    println!("deserialize_map");
    let mut keys = BTreeSet::new();
    let mut columns = Columns::new();
    Table::tables(sqlite)?.into_iter().try_for_each(|(_, table)| {
        if table.name() == "Map" || table.name() == "Vec" {
            columns.extend(table.get_children(&offset, None)?.into_iter().flat_map(|(id, values)| 
                Primitive::parse_many(values).map(|mut p| p.pop_first().map(|(k, (p, v))| (id, (p, v)))).transpose()
            ).collect::<Result<Columns>>()?);
        } else {
            keys.extend(table.get_children(&offset, Some(BTreeSet::new()))?.into_keys());
        }
        Ok::<_, Error>(())
    })?;
    keys.extend(columns.keys().map(|k| k.split(FS).collect::<Vec<_>>()[0].to_string()));
    Ok((keys, columns))
}

pub struct Deserializer<'a> {
    pub sqlite: &'a Connection,
    pub offset: PathBuf,
}

impl<'a> Deserializer<'a> {
    pub fn new<P: AsRef<Path>>(sqlite: &'a Connection, offset: P) -> Self {
        let mut offset = offset.as_ref().to_path_buf();
        if offset == PathBuf::new() {offset = PathBuf::from("/");}
        Deserializer{sqlite, offset}
    }

    fn deserialize_field(&self) -> Result<Option<(Type, Columns)>> {
        println!("deserializing search field");
        let mut parent = self.offset.components();
        let field = parent.next_back().ok_or(Error::RootPrimitive)?.as_os_str().to_string_lossy().to_string();
        Table::try_find_map(self.sqlite, |table| {
            let columns = table.columns().keys().filter_map(|c|
                c.starts_with(&format!("{field}{FS}")).then_some(c.to_string())
            ).collect::<BTreeSet<_>>();
            Ok(if !columns.is_empty() {table.get(&parent.as_path(), Some(columns))?.map(|c| (table.name().to_string(), c))} else {None})
        })?.map(|(name, c)|
            Ok((Type::from_str(&name, BTreeSet::from([field]))?, Primitive::parse_many(c)?))
        ).transpose()
    }

    fn deserialize_search<'de, V: Visitor<'de>>(self, visitor: V, option: bool) -> Result<V::Value> {
        println!("deserializing root search option: {option}");
        if self.offset.components().count() > 1 && let Some((ty, columns)) = self.deserialize_field()? {
            println!("field found");
            if let Some((n, (p, v))) = columns.iter().find(|(_, (_, v))| *v != Value::Null) {
                if option {
                    let n = n.split(FS).collect::<Vec<_>>()[0].to_string();
                    Compound::new(self.sqlite, self.offset, visitor, Type::Some(Box::new(ty), Some(columns), Some(n)))
                } else {
                    Compound::visit_value(p.clone(), v.clone(), visitor)
                }
            } else {
                if option {visitor.visit_none()} else {visitor.visit_unit()}
            }
        } else {
            if let Some((ty, columns)) = deserialize_type(self.sqlite, &self.offset)? {
                if option {
                    Compound::new(self.sqlite, self.offset, visitor, Type::Some(Box::new(ty), Some(columns), None))
                } else {
                    Compound{sqlite: self.sqlite, offset: self.offset, ty, columns: Some(columns), current_key: None}.visit(visitor)
                }
            } else {
                if option {visitor.visit_none()} else {visitor.visit_unit()}
            }
        }
    }

    fn deserialize_primitive<'de, V: Visitor<'de>>(self, primitive: Primitive, visitor: V) -> Result<V::Value> {
        let field = primitive.field(VALUE);
        if let Some(table) = Table::by_name(self.sqlite, NEW_TYPE_PRIMITIVE.to_string())?
        && let Some(mut columns) = table.get(&self.offset, Some(BTreeSet::from([field.clone()])))? {
            let value = columns.remove(&field).unwrap_or(Value::Null);
            Compound::visit_value(primitive, value, visitor)
        } else {
            let mut parent = self.offset.components();
            if parent.clone().count() < 3 {return Compound::visit_value(primitive, Value::Null, visitor);}
            let field = parent.next_back().unwrap().as_os_str().to_string_lossy().to_string();
            let pfield = primitive.field(&field);
            let (table, value) = Table::search_field(self.sqlite, &self.offset, pfield.clone())?.ok_or(Error::MissingRow(self.offset))?;
            Compound::visit_value(primitive, value, visitor)
        }
    }
}


impl<'de, 'a> serde::Deserializer<'de> for Deserializer<'a> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_search(visitor, false)
    }

    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_primitive(Primitive::bool, visitor)
    }

    fn deserialize_i8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_primitive(Primitive::i8, visitor)
    }

    fn deserialize_i16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_primitive(Primitive::i16, visitor)
    }

    fn deserialize_i32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_primitive(Primitive::i32, visitor)
    }

    fn deserialize_i64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_primitive(Primitive::i64, visitor)
    }

    fn deserialize_u8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_primitive(Primitive::u8, visitor)
    }
    fn deserialize_u16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_primitive(Primitive::u16, visitor)
    }
    fn deserialize_u32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_primitive(Primitive::u32, visitor)
    }
    fn deserialize_u64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_primitive(Primitive::u64, visitor)
    }
    fn deserialize_f32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_primitive(Primitive::f32, visitor)
    }
    fn deserialize_f64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_primitive(Primitive::f64, visitor)
    }
    fn deserialize_char<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_primitive(Primitive::char, visitor)
    }
    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_primitive(Primitive::String, visitor)
    }
    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_primitive(Primitive::String, visitor)
    }
    fn deserialize_bytes<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_primitive(Primitive::Blob, visitor)
    }
    fn deserialize_byte_buf<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_primitive(Primitive::Blob, visitor)
    }
  
    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_search(visitor, true)
    }

    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_unit()
    }
    fn deserialize_unit_struct<V: Visitor<'de>>(self, name: &'static str, visitor: V) -> Result<V::Value> {
        Compound::new(self.sqlite, self.offset, visitor, Type::Struct(name.to_string(), Default::default()))
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(self, name: &'static str, visitor: V) -> Result<V::Value> {
        Compound::new(self.sqlite, self.offset, visitor, Type::NewType(name.to_string()))
    }
    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        Compound::new(self.sqlite, self.offset, visitor, Type::Vec)
    }
    fn deserialize_tuple<V: Visitor<'de>>(self, len: usize, visitor: V) -> Result<V::Value> {
        Compound::new(self.sqlite, self.offset, visitor, Type::Tuple(0, len))
    }
    fn deserialize_tuple_struct<V: Visitor<'de>>(self, name: &'static str, len: usize, visitor: V) -> Result<V::Value> {
        Compound::new(self.sqlite, self.offset, visitor, Type::Struct(name.to_string(), (0..len).map(|i| i.to_string()).collect()))
    }
    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        Compound::new(self.sqlite, self.offset, visitor, Type::Map(Default::default()))
    }
    fn deserialize_struct<V: Visitor<'de>>(
        self, name: &'static str, fields: &'static [&'static str], visitor: V
    ) -> Result<V::Value> {
        Compound::new(self.sqlite, self.offset, visitor, Type::Struct(name.to_string(), fields.iter().map(|s| s.to_string()).collect()))
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self, name: &'static str, variants: &'static [&'static str], visitor: V,
    ) -> Result<V::Value> {
        Compound::new(self.sqlite, self.offset, visitor, Type::Enum(name.to_string(), "".to_string(), variants.iter().map(|f| f.to_string()).collect()))
    }

    fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        unimplemented!()//Any struct/enum gets turned into a Compound
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_any(visitor)
    }
}

#[derive(Debug, Clone)]
pub enum Type {
    Some(Box<Self>, Option<Columns>, Option<String>),
    NewType(String),
    Struct(String, BTreeSet<String>),
    Tuple(usize, usize),
    Enum(String, String, BTreeSet<String>),
    Map(BTreeSet<String>),
    Vec
}

impl Type {
    pub fn from_str(name: &str, fields: BTreeSet<String>) -> Result<Self> {
        let struc: Option<[&str; 2]> = name.split(VS).collect::<Vec<_>>().try_into().ok();
        Ok(if let Some([name, var]) = struc {
            Type::Enum(name.to_string(), var.to_string(), fields)
        } else { match name {
            NEW_TYPE_PRIMITIVE => Type::NewType("NewType".to_string()),
            "Map" => Type::Map(Default::default()),
            "Vec" => Type::Vec,
            //tuple => Type::Tuple(field.parse()?, len),
            other => Type::Struct(other.to_string(), fields)
        }})
    }
}

#[derive(Debug)]
pub struct Compound<'a> {
    sqlite: &'a Connection,
    offset: PathBuf,
    ty: Type,
    columns: Option<BTreeMap<String, (Primitive, Value)>>,
    current_key: Option<String>
}

impl<'a> Compound<'a> {
    //Given Type find columns then drive visitior
    pub fn new<'de, V: Visitor<'de>>(sqlite: &'a Connection, offset: PathBuf, visitor: V, mut ty: Type) -> Result<V::Value> {
        println!("deserializing new: {:?}", ty);
        let error = || Error::MissingRow(offset.clone());
        let (columns, current_key) = match ty {
            Type::Struct(ref name, ref fields) => {
                let table = Table::by_name(sqlite, name.clone())?.ok_or(error())?;
                let columns = table.columns().clone().into_keys().filter(|c| fields.iter().any(|f|
                    c.starts_with(&format!("{f}{FS}"))
                )).collect();
                (Some(Primitive::parse_many(table.get(&offset, Some(columns))?.ok_or(error())?)?), None)
            },
            Type::Some(ref ty, ref mut columns, ref mut field) => (columns.take(), field.take()),
            Type::NewType(ref name) => (None, None),
            Type::Tuple(_, len) => {
                let table = Table::by_name(sqlite, format!("({len})"))?.ok_or(error())?;
                (Some(Primitive::parse_many(table.get(&offset, None)?.ok_or(error())?)?), None)
            },
            Type::Map(ref mut keys) => {
                let (k, columns) = deserialize_map(sqlite, &offset)?;
                *keys = k;
                (Some(columns), None)
            }
            //Parse enum name for varinat and ensure its matches fields before searching for cols
            _ => todo!(),
        };
        Compound{sqlite, offset, ty, columns, current_key}.visit(visitor)
    }

    //Given Type modify based on previous Type, find columns,  then drive visitior
    pub fn deserialize_new<'de, V: Visitor<'de>>(&mut self, visitor: V, ty: Type) -> Result<V::Value> {
        //Both new type and some consume self in the next step
        let offset = self.current_key.as_ref().map(|k| self.offset.join(k)).unwrap_or(self.offset.clone());
        println!("deserializing old: {:?}, new: {:?}", self.ty, ty);
        println!("ncolumns: {:?}", self.columns);

        let new_name = if let Type::NewType(name) = &self.ty {Some(name.clone())} else {None};
        let keys = if let Type::Some(old_type, _, _) = &self.ty && let Type::Map(keys) = &**old_type {Some(keys.clone())} else {None};
        let (ty, columns) = match self.ty {
            Type::Some(_, _, _) | Type::NewType(_) => match ty {
                Type::NewType(name) => (Type::NewType(new_name.unwrap_or(name)), self.columns.take()),
                Type::Struct(name, fields) => {
                    let columns = self.columns.take().map(|c| c.into_iter().filter(|(k, _)| fields.contains(k)).collect());
                    (Type::Struct(new_name.unwrap_or(name), fields), columns)
                },
                Type::Map(_) => (Type::Map(keys.unwrap()), self.columns.take()),
                Type::Some(ty, mut columns, key) => (Type::Some(ty, None, key), columns.take()),
                ty => panic!("{:?}", ty)
            },
            _ => {
                let offset = self.offset.join(self.current_key.take().unwrap());
                return Self::new(self.sqlite, offset, visitor, ty);
            }
        };
        match columns {
            Some(columns) => Compound{
                sqlite: self.sqlite, offset: self.offset.clone(), ty,
                columns: Some(columns), current_key: None
            }.visit(visitor),
            None => Self::new(self.sqlite, self.offset.clone(), visitor, ty)
        }
    }

    fn visit<'de, V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value> {
        println!("visit: {:?}", self.ty);
        match self.ty {
            Type::Struct(_, _) | Type::Map(_) => visitor.visit_map(self),
            Type::Some(_, _, _) => visitor.visit_some(&mut self),
            Type::NewType(_) => visitor.visit_newtype_struct(&mut self),
            Type::Tuple(_, _) | Type::Vec => visitor.visit_seq(self),
            Type::Enum(_, _, _) => visitor.visit_enum(self)
        }
    }
    
    fn deserialize_search<'de, V: Visitor<'de>>(&mut self, visitor: V, option: bool) -> Result<V::Value> {
        println!("deserializing search optional: {option}");
        //if matches!(Type::Map(_), self.ty) {
            
        //} else
        if let Type::NewType(name) = &self.ty {
            println!("deserializing newtype search");
            if let Some(table) = Table::by_name(self.sqlite, NEW_TYPE_PRIMITIVE.to_string())? &&
            let Some(columns) = table.get(&self.offset, None)? {
                let columns = Primitive::parse_many(columns)?;
                if let Some((n, (p, v))) = columns.iter().find(|(_, (_, v))| *v != Value::Null) {
                    if option {
                        self.current_key = Some(n.split(FS).collect::<Vec<_>>()[0].to_string());
                        self.columns = Some(columns);
                        visitor.visit_some(self)
                    } else {
                        Compound::visit_value(p.clone(), v.clone(), visitor)
                    }
                } else {
                    if option {visitor.visit_none()} else {visitor.visit_unit()}
                }
            } else {
                println!("searching for table");
                if let Some(table) = Table::by_name(self.sqlite, name.to_string())? && let Some(c) = table.get(&self.offset, None)? {
                    let columns = Primitive::parse_many(c)?;
                    println!("scolumns: {:?}", columns);
                    self.deserialize_new(visitor, Type::Some(
                        Box::new(Type::from_str(&name, columns.keys().cloned().collect())?), Some(columns), None
                    ))
                } else {
                    if option {visitor.visit_none()} else {visitor.visit_unit()}
                }
            }
        } else {
            println!("non newtype search");
            let key = self.current_key.as_ref().unwrap();
            if let Some(k) = self.columns.as_ref().and_then(|c| c.keys().find_map(|k| (k == key).then_some(k.clone()))) {
                println!("found field");
                if option {
                    visitor.visit_some(self)
                } else {
                    self.current_key = None; 
                    let (p, v) = self.columns.as_mut().unwrap().remove(&k).unwrap();
                    if v != Value::Null {
                        Self::visit_value(p, v, visitor)
                    } else {visitor.visit_unit()}
                } 
            } else {
                let offset = self.offset.join(key);
                println!("searching for table");
                if let Some((ty, mut columns)) = deserialize_type(self.sqlite, &offset)? {
                    println!("found columns: {:?}", columns);
                    if option {
                        self.deserialize_new(visitor, Type::Some(Box::new(ty), Some(columns), None))
                    } else {
                        self.current_key = None;
                        if let Type::NewType(_) = ty && let Some((p, v)) = columns.remove(VALUE) {
                            Self::visit_value(p.clone(), v.clone(), visitor)
                        } else {
                            Compound{sqlite: self.sqlite, offset, ty, columns: Some(columns), current_key: None}.visit(visitor)
                        }
                    }
                } else {
                    println!("found none");
                    self.current_key = None;
                    if option {visitor.visit_none()} else {visitor.visit_unit()}
                }
            }
        }
    }

    pub fn has_next_key(&self) -> bool {
        match &self.ty {
            Type::Struct(_, fields) | Type::Map(fields) => !fields.is_empty(),
            Type::Tuple(i, len) => i < len,
            _ => todo!()
        }
    }

    pub fn next_key(&mut self) -> Result<String> {
        Ok(match &mut self.ty {
            Type::Struct(_, fields) | Type::Map(fields) => {
                let key = fields.pop_first().unwrap();
                self.current_key = Some(key.clone());
                key
            },
            Type::Tuple(i, len) => {
                let key = format!("{i}");
                self.current_key = Some(key.clone());
                *i += 1;
                key
            }
            _ => todo!()
        })
    }

    fn deserialize_value<'de, V: Visitor<'de>>(&mut self, primitive: Primitive, visitor: V) -> Result<V::Value> {
        println!("deserializing value: {primitive:?}");
        let name = self.current_key.take().expect("no current_key");
        println!("vcolumns: {:?}", self.columns);
        println!("currentk key: {:?}", name);
        let (primitive, value) = self.columns.as_mut().unwrap().remove(&name).unwrap();
        println!("{:?}", (&primitive, &value));
        Self::visit_value(primitive, value, visitor)
    }

    fn visit_value<'de, V: Visitor<'de>>(primitive: Primitive, value: Value, visitor: V) -> Result<V::Value> {
        println!("inner {:?}", (&primitive, &value));
        match (primitive, value) {
            (Primitive::bool, Value::Integer(i)) => visitor.visit_bool(i != 0),
            (Primitive::i8, Value::Integer(i)) => visitor.visit_i8(i as i8),
            (Primitive::i16, Value::Integer(i)) => visitor.visit_i16(i as i16),
            (Primitive::i32, Value::Integer(i)) => visitor.visit_i32(i as i32),
            (Primitive::i64, Value::Integer(i)) => visitor.visit_i64(i),
            (Primitive::u8, Value::Integer(i)) => visitor.visit_u8(i as u8),
            (Primitive::u16, Value::Integer(i)) => visitor.visit_u16(i as u16),
            (Primitive::u32, Value::Integer(i)) => visitor.visit_u32(i as u32),
            (Primitive::u64, Value::Integer(i)) => visitor.visit_u64(i as u64),
            (Primitive::f32, Value::Real(r)) => visitor.visit_f32(r as f32),
            (Primitive::f64, Value::Real(r)) => visitor.visit_f64(r),
            (Primitive::char, Value::Text(c)) => visitor.visit_char(c.chars().next().expect("empty char string")),
            (Primitive::String, Value::Text(s)) => visitor.visit_string(s),
            (Primitive::Blob, Value::Blob(b)) => visitor.visit_bytes(&b),
            (ty, Value::Null) => Err(Error::invalid_type(Unexpected::Other("Null"), &format!("{:?}", &ty).as_str())),
            (ty, value) => Err(Error::invalid_value(Unexpected::Other(
                format!("(name: {:?})", &value).as_str()
            ), &format!("{:?}", &ty).as_str()))
        }
    }
}

impl<'de, 'a> serde::Deserializer<'de> for &mut Compound<'a> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value> {
        if let Type::Some(ty, _, _) = self.ty.clone() {
            Compound{sqlite: self.sqlite, offset: self.offset.clone(), ty: *ty, columns: self.columns.take(), current_key: None}.visit(visitor)
        } else {
            self.deserialize_search(visitor, false)
        }
    }

    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_value(Primitive::bool, visitor)
    }

    fn deserialize_i8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_value(Primitive::i8, visitor)
    }

    fn deserialize_i16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_value(Primitive::i16, visitor)
    }

    fn deserialize_i32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_value(Primitive::i32, visitor)
    }

    fn deserialize_i64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_value(Primitive::i64, visitor)
    }

    fn deserialize_u8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_value(Primitive::u8, visitor)
    }
    fn deserialize_u16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_value(Primitive::u16, visitor)
    }
    fn deserialize_u32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_value(Primitive::u32, visitor)
    }
    fn deserialize_u64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_value(Primitive::u64, visitor)
    }
    fn deserialize_f32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_value(Primitive::f32, visitor)
    }
    fn deserialize_f64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_value(Primitive::f64, visitor)
    }
    fn deserialize_char<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_value(Primitive::char, visitor)
    }
    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        println!("deserializing string");
        if self.current_key.is_none() {
            let key = self.next_key()?;
            println!("deserializing string as identifier: {key}");
            visitor.visit_string(key)
        } else {
            self.deserialize_value(Primitive::String, visitor)
        }
    }
    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_str(visitor)
    }
    fn deserialize_bytes<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_value(Primitive::Blob, visitor)
    }
    fn deserialize_byte_buf<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_value(Primitive::Blob, visitor)
    }
  
    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        if matches!(self.ty, Type::Some(_, _, _)) {
            visitor.visit_some(self)//We already found the next step so just Some it up
        } else {
            self.deserialize_search(visitor, true)
        }
    }

    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_unit()
    }
    fn deserialize_unit_struct<V: Visitor<'de>>(self, name: &'static str, visitor: V) -> Result<V::Value> {
        self.deserialize_new(visitor, Type::Struct(name.to_string(), Default::default()))
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(self, name: &'static str, visitor: V) -> Result<V::Value> {
        self.deserialize_new(visitor, Type::NewType(name.to_string()))
    }
    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_new(visitor, Type::Vec)
    }
    fn deserialize_tuple<V: Visitor<'de>>(self, len: usize, visitor: V) -> Result<V::Value> {
        self.deserialize_new(visitor, Type::Tuple(0, len))
    }
    fn deserialize_tuple_struct<V: Visitor<'de>>(self, name: &'static str, len: usize, visitor: V) -> Result<V::Value> {
        self.deserialize_new(visitor, Type::Struct(name.to_string(), (0..len).map(|i| i.to_string()).collect()))
    }
    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_new(visitor, Type::Map(Default::default()))
    }
    fn deserialize_struct<V: Visitor<'de>>(
        self, name: &'static str, fields: &'static [&'static str], visitor: V
    ) -> Result<V::Value> {
        self.deserialize_new(visitor, Type::Struct(name.to_string(), fields.iter().map(|f| f.to_string()).collect()))
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self, name: &'static str, variants: &'static [&'static str], visitor: V,
    ) -> Result<V::Value> {
        self.deserialize_new(visitor, Type::Enum(name.to_string(), "".to_string(), variants.iter().map(|f| f.to_string()).collect()))
    }

    fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let key = self.next_key()?;
        println!("identifier: {:?}", key);
        visitor.visit_string(key)
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_any(visitor)
    }
}

impl<'de, 'a> MapAccess<'de> for Compound<'a> {
    type Error = Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(&mut self, seed: K) -> Result<Option<K::Value>> {
        println!("next_key_seed");
        self.has_next_key().then(|| seed.deserialize(self)).transpose()
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value> {
        println!("next_value_seed");
        seed.deserialize(self)
    }
}


impl<'de, 'a> VariantAccess<'de> for Compound<'a> {
    type Error = Error;

    fn unit_variant(self) -> Result<()> {
        todo!()
    }
    fn newtype_variant_seed<T: DeserializeSeed<'de>>(mut self, seed: T) -> Result<T::Value> {
        seed.deserialize(&mut self)
    }
    fn tuple_variant<V: Visitor<'de>>(mut self, len: usize, visitor: V) -> Result<V::Value> {
        self.deserialize_new(visitor, Type::Tuple(0, len))
    }

    fn struct_variant<V: Visitor<'de>>(self, fields: &'static [&'static str], visitor: V) -> Result<V::Value> {
        todo!()
        //self.deserialize_new(visitor, Type::Struct())
    }
}

impl<'de, 'a> EnumAccess<'de> for Compound<'a> {
    type Error = Error;
    type Variant = Compound<'a>;

    fn variant_seed<V: DeserializeSeed<'de>>(mut self, seed: V) -> Result<(V::Value, Self::Variant)> {
        let variant = seed.deserialize(&mut self)?;
        Ok((variant, self))
    }
}

impl<'de, 'a> SeqAccess<'de> for Compound<'a> {
    type Error = Error;

    fn next_element_seed<T: DeserializeSeed<'de>>(&mut self, seed: T) -> Result<Option<T::Value>> {
        self.has_next_key().then(|| {
            self.next_key()?;
            seed.deserialize(self)
        }).transpose()
    }
}
