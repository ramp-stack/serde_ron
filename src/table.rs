use rusqlite::{Connection, Transaction, TransactionBehavior, Error, types};
use types::{FromSql, FromSqlResult, FromSqlError};

use std::collections::{BTreeMap, BTreeSet};
use std::path::{PathBuf, Path};
use std::str::FromStr;

const ID: &str = "ID";
const PARENT: &str = "PARENT";
///Used as the column name for NewType and Map entries
pub const VALUE: &str = "VALUE";
const MAP: &str = "MAP__";

const FS: &str = ": ";
const VS: &str = "::";

fn missing(n: &str) -> Error {Error::InvalidColumnName(format!("Missing Column: \"{n}\""))}
fn invalid_table(n: &str) -> Error {Error::InvalidColumnName(format!("Invalid Table Name: \"{n}\""))}

trait OptionalExtension<T> {
    fn optional(self) -> Result<Option<T>, Error>;
}

impl<T> OptionalExtension<T> for Result<T, Error> {
    fn optional(self) -> Result<Option<T>, Error> {
        match self {
            Err(Error::SqliteFailure(_, Some(msg))) if msg.contains("no such table") => Ok(None),
            Err(Error::QueryReturnedNoRows) => Ok(None),
            Ok(t) => Ok(Some(t)),
            Err(e) => Err(e)
        }
    }
}

#[allow(non_camel_case_types)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Primitive {
    bool, i8, i16, i32, i64, u8, u16, u32, u64, f32, f64, char, String, Bytes
}

impl Primitive {
    fn to_type(&self) -> types::Type {
        match self {
            Self::bool | Self::i8 | Self::i16 | Self::i32 | Self::i64 | Self::u8 | Self::u16 | Self::u32 | Self::u64 
                => types::Type::Integer,
            Self::f32 | Self::f64 => types::Type::Real,
            Self::char | Self::String => types::Type::Text,
            Self::Bytes => types::Type::Blob,
        }
    }
}

impl FromStr for Primitive {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "bool" => Self::bool, 
            "i8" => Self::i8, 
            "i16" => Self::i16, 
            "i32" => Self::i32, 
            "i64" => Self::i64, 
            "u8" => Self::u8, 
            "u16" => Self::u16, 
            "u32" => Self::u32, 
            "u64" => Self::u64, 
            "f32" => Self::f32, 
            "f64" => Self::f64, 
            "char" => Self::char, 
            "String" => Self::String, 
            "Vec<u8>" => Self::Bytes, 
            _ => {return Err(Error::InvalidColumnName(s.to_string()));}
        })
    }
}
impl std::fmt::Display for Primitive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {match self {
        Self::Bytes => write!(f, "Vec<u8>"),
        s => write!(f, "{s:?}")
    }}
}

#[allow(non_camel_case_types)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Type {
    Primitive(bool, Primitive),
    Table(bool, String),
    Null
}

impl Type {
    fn to_type(&self) -> types::Type {match self {
        Self::Primitive(_, p) => p.to_type(),
        _ => types::Type::Null,
    }}
    fn nullable(&self) -> bool {match self {
        Self::Primitive(n, _) => *n,
        Self::Table(n, _) => *n,
        _ => true
    }}

    fn from_str(s: &str) -> Result<(String, Self), Error> {
        let [field, ty]: [&str; 2] = s.split(FS).collect::<Vec<_>>().try_into().map_err(|_| Error::InvalidColumnName(s.to_string()))?;
        Ok((field.to_string(), if ty == "Null" {Self::Null} else {
            let (nullable, p) = ty.strip_prefix("Option<").and_then(|t| t.strip_suffix(">")).map(|o| (true, o)).unwrap_or((false, ty));
            Primitive::from_str(p).ok().map(|p| Self::Primitive(nullable, p)).unwrap_or(Self::Table(nullable, p.to_string()))
        }))
    }
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {match self {
        Self::Primitive(nullable, p) => if *nullable {write!(f, "Option<{p}>")} else {write!(f, "{p}")},
        Self::Table(nullable, name) => if *nullable {write!(f, "Option<{name}>")} else {write!(f, "{name}")},
        Self::Null => write!(f, "Null"),
    }}
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    bool(bool), i8(i8), i16(i16), i32(i32), i64(i64), u8(u8), u16(u16), u32(u32),
    u64(u64), f32(f32), f64(f64), char(char), String(String), Bytes(Vec<u8>), 
    Null, Table(String), Some(Box<Self>)
}
impl Value {
    pub fn some(val: Value) -> Self {Value::Some(Box::new(val))}

    fn from_value(index: usize, ty: &Type, value: types::Value) -> Result<Self, Error> {
        let w = |e| Error::FromSqlConversionFailure(index, types::Type::Null, Box::new(e));
        Ok(match ty {
            Type::Table(nullable, name) => Value::Table(name.to_string()),
            Type::Null => Value::Null,
            Type::Primitive(false, _) if value == types::Value::Null => {
                return Err(w(FromSqlError::InvalidType));
            },
            _ if value == types::Value::Null => Self::Null,
            Type::Primitive(nullable, primitive) => {
                let v = match primitive {
                    Primitive::bool => Self::bool(bool::column_result((&value).into()).map_err(w)?),
                    Primitive::i8 => Self::i8(i8::column_result((&value).into()).map_err(w)?),
                    Primitive::i16 => Self::i16(i16::column_result((&value).into()).map_err(w)?),
                    Primitive::i32 => Self::i32(i32::column_result((&value).into()).map_err(w)?),
                    Primitive::i64 => Self::i64(i64::column_result((&value).into()).map_err(w)?),
                    Primitive::u8 => Self::u8(u8::column_result((&value).into()).map_err(w)?),
                    Primitive::u16 => Self::u16(u16::column_result((&value).into()).map_err(w)?),
                    Primitive::u32 => Self::u32(u32::column_result((&value).into()).map_err(w)?),
                    Primitive::u64 => Self::u64(u64::column_result((&value).into()).map_err(w)?),
                    Primitive::f32 => Self::f32(f32::column_result((&value).into()).map_err(w)?),
                    Primitive::f64 => Self::f64(f64::column_result((&value).into()).map_err(w)?),
                    Primitive::char => Self::char(String::column_result((&value).into()).map_err(w)?.chars().next().ok_or(w(FromSqlError::InvalidType))?),
                    Primitive::String => Self::String(String::column_result((&value).into()).map_err(w)?),
                    Primitive::Bytes => Self::Bytes(Vec::<u8>::column_result((&value).into()).map_err(w)?),
                };
                if *nullable {Self::Some(Box::new(v))} else {v}
            }
        })
    }

    pub fn to_type(&self) -> Type {match self {
        Self::Null => Type::Null,
        Self::Table(name) => Type::Table(false, name.to_string()),
        Self::bool(b) => Type::Primitive(false, Primitive::bool),
        Self::i8(b) => Type::Primitive(false, Primitive::i8),
        Self::i16(b) => Type::Primitive(false, Primitive::i16),
        Self::i32(b) => Type::Primitive(false, Primitive::i32),
        Self::i64(b) => Type::Primitive(false, Primitive::i64),
        Self::u8(b) => Type::Primitive(false, Primitive::u8),
        Self::u16(b) => Type::Primitive(false, Primitive::u16),
        Self::u32(b) => Type::Primitive(false, Primitive::u32),
        Self::u64(b) => Type::Primitive(false, Primitive::u64),
        Self::f32(b) => Type::Primitive(false, Primitive::f32),
        Self::f64(b) => Type::Primitive(false, Primitive::f64),
        Self::char(b) => Type::Primitive(false, Primitive::char),
        Self::String(b) => Type::Primitive(false, Primitive::String),
        Self::Bytes(b) => Type::Primitive(false, Primitive::Bytes),
        Self::Some(t) => match t.to_type() {
            Type::Primitive(_, p) => Type::Primitive(true, p),
            Type::Table(_, p) => Type::Table(true, p),
            t => t
        }
    }}

    fn to_value(self) -> types::Value {
        match self {
            Self::bool(b) => types::Value::Integer(b as i64),
            Self::i8(i) => types::Value::Integer(i as i64),
            Self::i16(i) => types::Value::Integer(i as i64),
            Self::i32(i) => types::Value::Integer(i as i64), 
            Self::i64(i) => types::Value::Integer(i),
            Self::u8(u) => types::Value::Integer(u as i64),
            Self::u16(u) => types::Value::Integer(u as i64),
            Self::u32(u) => types::Value::Integer(u as i64),
            Self::u64(u) => types::Value::Integer(u as i64),
            Self::f32(f) => types::Value::Real(f as f64), 
            Self::f64(f) => types::Value::Real(f),
            Self::char(c) => types::Value::Text(c.to_string()), 
            Self::String(s) => types::Value::Text(s),
            Self::Bytes(b) => types::Value::Blob(b),
            Self::Null | Self::Table(_) => types::Value::Null,
            Self::Some(t) => t.to_value()
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.clone().to_value() {
            types::Value::Null => write!(f, "NULL"),
            types::Value::Integer(i) => write!(f, "{i}"),
            types::Value::Real(r) => write!(f, "{r}"),
            types::Value::Text(t) => write!(f, "'{t}'"),
            types::Value::Blob(b) => write!(f, "x\"{}\"", hex::encode(b)),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Schema {
    Variant(String, Box<Self>),
    Struct(BTreeMap<String, Type>),
    NewType(Type),
    Map(Type)
}

impl Schema {
    fn from(name: &str, mut columns: BTreeMap<String, Type>) -> Result<(String, Self), Error> {
        match name.find(":") {
            Some(i) => {
                let split = name.split_at(i);
                if split.1 == ": Map" {
                    Ok((split.0.to_string(), Schema::Map(columns.remove(VALUE).ok_or(missing(VALUE))?)))
                } else if let Some(variant) = split.1.strip_prefix(VS) {
                    let (variant, t) = Schema::from(variant, columns)?;
                    Ok((split.0.to_string(), Schema::Variant(variant.to_string(), Box::new(t))))
                } else {
                    Err(invalid_table(name))
                }
            },
            None => {
                let schema = if columns.keys().all(|n| n == VALUE) {
                    Schema::NewType(columns.remove(VALUE).unwrap())
                } else {Schema::Struct(columns)};
                Ok((name.to_string(), schema))
            }
        }
    }

    pub fn name(&self, name: &str) -> String {match self {
        Self::Variant(variant, inner) => inner.name(&format!("{name}::{variant}")),
        Self::Map(_) => format!("{name}: Map"),
        _ => name.to_string(),
    }}

    fn into_columns(self) -> BTreeMap<String, Type> {match self {
        Self::Variant(_, inner) => inner.into_columns(),
        Self::Struct(columns) => columns,
        Self::Map(value) | Self::NewType(value) => BTreeMap::from([(VALUE.to_string(), value)]),
    }}

    fn columns(&self) -> BTreeMap<String, &Type> {match self {
        Self::Variant(_, inner) => inner.columns(),
        Self::Struct(columns) => columns.iter().map(|(k, v)| (k.to_string(), v)).collect(), 
        Self::Map(value) | Self::NewType(value) => BTreeMap::from([(VALUE.to_string(), value)]),
    }}

    fn retype(&mut self, n: String, t: Type) {match self {
        Self::Variant(_, inner) => inner.retype(n, t),
        Self::Struct(columns) => {columns.insert(n, t);},
        Self::Map(p) | Self::NewType(p) => {*p = t;}
    }}
}

#[derive(Clone, Debug)]
pub struct Table {
    name: String,
    schema: Schema,
}

impl Table {
    pub fn schema(&self) -> &Schema {&self.schema}
    pub fn name(&self) -> String {self.schema.name(&self.name)}

    pub fn from_name(connection: &Connection, name: &str) -> Result<Option<Self>, Error> {
        let cmd = format!("SELECT cid, name, type, \"notnull\" FROM pragma_table_info('{name}')");
        let columns = connection.prepare(&cmd)?.query_map([], |row| {
            let cid = row.get(0)?;
            let field: String = row.get(1)?;
            let sty = match row.get::<_, String>(2)?.to_uppercase().as_str() {
                "INTEGER" => types::Type::Integer,
                "REAL" => types::Type::Real,
                "TEXT" => types::Type::Text,
                "BLOB" => types::Type::Blob,
                "" => types::Type::Null,
                unknown => {return Err(Error::InvalidColumnType(2, format!("Unknown Type: {unknown}"), types::Type::Null));}
            };
            let nullable: bool = !row.get(3)?;
            //It might not be possible to create a Null type column that is NOT NULL
            //if ty == Type::Null && not_null {return Err(Error::InvalidcolumnType(cid, field, types::Type::Null));}
            let (name, ty) = if field == ID {
                (field.clone(), Type::Primitive(false, Primitive::String))
            } else {
                Type::from_str(&field)?
            };
            if nullable != ty.nullable() {return Err(Error::InvalidColumnType(cid, field, types::Type::Null));}
            if sty != ty.to_type() {return Err(Error::InvalidColumnType(cid, field, sty));}
            Ok((name, ty))
        })?.collect::<Result<BTreeMap<String, Type>, Error>>().optional()?.filter(|c| !c.is_empty());
        let mut columns = if let Some(c) = columns {c} else {return Ok(None);};
        if columns.remove(ID).is_none() {return Err(missing(ID));}

        let (name, schema) = Schema::from(name, columns)?;
        Ok(Some(Table{name, schema}))
    }

    pub fn new(connection: &Connection, name: String, schema: Schema) -> Result<Self, Error> {
        let table_name = schema.name(&name);
        Ok(match Table::from_name(connection, &table_name)? {
            None => {
                let params = schema.columns().iter().enumerate().map(|(i, (n, p))| format!("\"{n}{FS}{p}\" {}{}",
                    p.to_type().to_string().to_uppercase(), if p.nullable() {String::new()} else {" NOT NULL".to_string()}
                )).collect::<Vec<_>>().join(", ");
                let params = if params.is_empty() {String::new()} else {format!(", {params}")};
                let cmd = format!("CREATE TABLE IF NOT EXISTS \"{table_name}\"(
                    {ID} TEXT PRIMARY KEY NOT NULL{params}
                )");
                println!("cmd: {:?}", cmd);
                connection.execute(&cmd, [])?;
              //let cmd = format!("CREATE INDEX IF NOT EXISTS \"index_{name}_{PARENT}\" ON \"{name}\" (\"{PARENT}\");");
              //connection.execute(&cmd, [])?;
                Table{name, schema}
            },
            Some(mut table) => {
                table.check_schema(connection, schema.into_columns(), true)?;
                table
            },
        })
    }

    fn check_schema(&mut self, connection: &Connection, mut columns: BTreeMap<String, Type>, exaustive: bool) -> Result<(), Error> {
        let retypes = self.schema.columns().iter().enumerate().flat_map(|(i, (n, p))| match (p, columns.remove(n)) {
            (_, None) if exaustive => Some(Err(missing(n))),
            (Type::Null, Some(new)) if new != Type::Null => if new.nullable() {
                Some(Ok((n.to_string(), new)))
            } else {Some(Err(Error::InvalidColumnType(i+1, format!("{n}{FS}{new}"), types::Type::Null)))},
            (old, Some(new)) if **old != new && old.nullable() != new.nullable() =>
                Some(Err(Error::InvalidColumnType(i+1, format!("{n}{FS}{new}"), types::Type::Null))),
            (old, Some(new)) if **old != new && new != Type::Null =>
                Some(Err(Error::InvalidColumnType(i+1, format!("{n}{FS}{new}"), new.to_type()))),
            _ => None
        }).collect::<Result<BTreeMap<String, Type>, Error>>()?;
        if let Some((n, p)) = columns.pop_first() {return Err(Error::InvalidColumnName(n.to_string()));}
        retypes.into_iter().try_for_each(|(n, t)| self.retype(connection, n, t))?;
        Ok(())
    }

    fn retype(&mut self, connection: &Connection, n: String, t: Type) -> Result<(), Error> {
        let name = self.name();
        let cmd = format!("ALTER TABLE \"{name}\" DROP COLUMN \"{n}: Null\";");
        println!("cmd: {:?}", cmd);
        connection.execute(&cmd, [])?;
        let cmd = format!("ALTER TABLE \"{name}\" ADD COLUMN \"{n}{FS}{t}\" {} {};",
            t.to_type().to_string().to_uppercase(),
            if t.nullable() {String::new()} else {" NOT NULL".to_string()}
        );
        println!("cmd: {:?}", cmd);
        connection.execute(&cmd, [])?;
        self.schema.retype(n, t);
        
        Ok(())
    }

    pub fn tables(connection: &Connection) -> Result<BTreeMap<String, Self>, Error> {
        let cmd = "SELECT name FROM sqlite_schema WHERE type = 'table' AND name NOT LIKE 'sqlite_%';";
        connection.prepare(cmd)?.query_map([], |row| {
            let name = row.get::<_, String>(0)?;
            let table = Table::from_name(connection, &name)?.unwrap();
            Ok((name, table))
        })?.collect()
    }

    pub fn insert<P: AsRef<Path>>(&mut self, connection: &Connection, path: P, values: BTreeMap<String, Value>) -> Result<(), Error> {
        let columns = match &self.schema {
            Schema::Map(t) => {
                let ty = values.iter().enumerate().try_fold(Type::Null, |acc, (i, (k, v))| {
                    let t = v.to_type();
                    if acc != Type::Null && t != acc && t != Type::Null {Err(Error::InvalidColumnType(i, format!("{k}{FS}{}", t), t.to_type()))} else {Ok(t)}
                })?;
                BTreeMap::from([(VALUE.to_string(), ty)])
            },
            _ => values.iter().map(|(s, v)| (s.to_string(), v.to_type())).collect()
        };
        self.check_schema(connection, columns, false)?;
        let columns = self.schema.columns();

        let inserts = match &self.schema {
            Schema::Map( t) => {
                values.into_iter().map(|(k, v)| (path.as_ref().join(k), BTreeMap::from([(format!("{VALUE}{FS}{t}"), v)]))).collect()
            },
            _ => vec![(path.as_ref().to_path_buf(), values.into_iter().map(|(n, v)| (format!("{n}{FS}{}", columns.get(&n).unwrap()), v)).collect())]
        };
        println!("inserts: {:?}", inserts);

        for (id, columns) in inserts {
            let ((n, u), v): ((Vec<_>, Vec<_>), Vec<_>) = columns.into_iter().map(|(n, v)| {
                ((format!("\"{n}\""), format!("'{n}'=excluded.'{n}'")), v.to_string())
            }).unzip();

            let name = self.name();
            let (n, v, u) = if !n.is_empty() {(
                format!(", {}", n.join(",")), format!(", {}", v.join(",")), format!("UPDATE SET {}", u.join(","))
            )} else { Default::default()};
            let u = if u.is_empty() {"NOTHING".to_string()} else {u};

            let id = id.to_string_lossy();
            let cmd = format!("INSERT INTO \"{name}\"({ID}{n}) VALUES ('{id}'{v}) ON CONFLICT DO {u};");
            println!("cmd: {:?}", cmd);
            connection.execute(&cmd, [])?;
        }
      
        Ok(())
    }

    pub fn get<P: AsRef<Path>>(&self, connection: &Connection, id: P, columns: Option<BTreeSet<String>>) -> Result<Option<BTreeMap<String, Value>>, Error> {
        let id = id.as_ref().to_string_lossy().to_string();
        let name = self.name();
        match &self.schema {
            Schema::Map(t) => {
                let col = (*t != Type::Null).then_some(format!("{VALUE}{FS}{t}"));
                let param = col.as_ref().map(|col| format!(", \"{col}\"")).unwrap_or_default();
                let id = if id == "/" {id} else {format!("{id}/")}; 
                let cmd = format!("SELECT {ID}{param} FROM \"{name}\" WHERE {ID} LIKE '{id}%' AND id NOT LIKE '{id}%/%'");
                println!("cmd: {:?}", cmd);
                connection.prepare(&cmd)?.query_map([], |row| Ok((
                    row.get::<&str, String>(ID)?.strip_prefix(&id).unwrap().to_string(),
                    col.as_ref().map(|col| Value::from_value(1, &t, row.get::<&str, types::Value>(col)?)).unwrap_or(Ok(Value::Null))?
                ))).optional()?.map(|t| t.collect()).transpose()
            },
            _ => {
                let schema_columns = self.schema.columns();
                let columns = if let Some(columns) = columns {
                    columns.into_iter().map(|n| {
                        let ty = schema_columns.get(&n).ok_or(Error::InvalidColumnName(n.to_string()))?;
                        Ok((n, ty))
                    }).collect::<Result<BTreeMap<_, _>, Error>>()?
                } else {schema_columns.iter().flat_map(|(n, ty)|
                    (!matches!(ty.to_type(), types::Type::Null)).then_some((n.to_string(), ty))
                ).collect()};

                let param = columns.iter().flat_map(|(n, t)|
                    (!matches!(t.to_type(), types::Type::Null)).then_some(format!("\"{n}{FS}{t}\""))
                ).collect::<Vec<_>>().join(",");
                let param = if param.is_empty() {param} else {format!(", {param}")};
                let cmd = format!("SELECT {ID}{param} FROM \"{name}\" WHERE {ID} = '{id}'");
                println!("cmd: {:?}", cmd);
                connection.query_row(&cmd, [], |row| {
                    row.get::<usize, String>(0)?;
                    columns.into_iter().enumerate().map(|(i, (name, ty))| match ty.to_type() {
                        types::Type::Null => Ok((name, Value::Null)),
                        _ => Ok((name, Value::from_value(i+1, ty, row.get::<usize, types::Value>(i+1)?)?))
                    }).collect::<Result<BTreeMap<String, Value>, Error>>()
                }).optional()
            }
        }
    }

    pub fn delete<P: AsRef<Path>>(&self, connection: &Connection, path: P) -> Result<(), Error> {
        let id = path.as_ref().to_string_lossy();
        let name = self.name();
        let cmd = format!("DELETE FROM \"{name}\" WHERE {ID} LIKE '{id}%'");
        let i = connection.execute(&cmd, [])?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Structure {
    Variant(String, Box<Self>),
    Struct(BTreeMap<String, Value>),
    NewType(Value),
    Map(BTreeMap<String, Value>),
    Null
}
impl Structure {
    pub fn latest(&mut self) -> &mut Self {match self {
        Self::Variant(_, b) => &mut **b,
        l => l
    }}
}

#[derive(Debug, Clone, PartialEq)]
pub enum Data {
    //Collection(BTreeMap<String, Value>),
    Structure(String, Structure),
    Primitive(Value)
}

#[derive(Debug)]
pub struct TableAccess<'a> {
    transaction: Transaction<'a>,
    tables: BTreeMap<String, Table>,
    cache: BTreeMap<PathBuf, (String, String)>,
}
impl<'a> TableAccess<'a> {
    pub fn new(connection: &'a mut Connection) -> Result<Self, Error> {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Exclusive)?;
        Ok(TableAccess{
            tables: Table::tables(&transaction)?,
            transaction,
            cache: BTreeMap::new()
        })
    }

    pub fn insert<P: AsRef<Path>>(&mut self, offset: P, data: Data) -> Result<(), Error> {
        panic!("data: {data:?}");
      //let table_name = schema.name(&name);
      //let table = match self.tables.get(&table_name) {
      //    None => self.tables.insert(table_name, Table::new(&self.transaction, name, schema)?),
      //    Some(t) => t
      //};
      //if !parent {
      //    self.tables.iter().map(|
      //}
        //1. Create/Get table
        //2. Read id and if not present delete from everywhere
        //3. Write to table
        //
        ////
        //
        //1. Search for parent table
        //2. Update parent table schema with child info if needed
        //3. Read from table and if not present delete from everywhere
        //4. Write Table
        //
        //
        //1. Search all tables for parent
        //2. Verify value with table schema
        //3. If found write to row with value
    }

    pub fn delete<P: AsRef<Path>>(&mut self, path: P) -> Result<(), Error> {
        //TODO: could be primitive field
        self.tables.values().try_for_each(|table| table.delete(&self.transaction, path.as_ref()))
    }

    pub fn commit(self) -> Result<(), Error> {
        self.transaction.commit()
    }
}
