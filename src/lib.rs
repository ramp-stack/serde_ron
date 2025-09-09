use rusqlite::{Connection, Error};
use rusqlite::fallible_iterator::FallibleIterator;

pub use active_base::*;

use std::collections::{BTreeSet, BTreeMap};
use std::path::PathBuf;
use std::cmp::Ordering;

trait OptionalExtension<T> {
    fn optional(self) -> Result<Option<T>, Error>;
}

impl<T> OptionalExtension<T> for Result<T, Error> {
    fn optional(self) -> Result<Option<T>, Error> {
        match self {
            Err(Error::SqliteFailure(rusqlite::ffi::Error { code: rusqlite::ErrorCode::Unknown, extended_code: 1 }, Some(msg))) if msg.strip_prefix("no such table").is_some() => Ok(None),
            Err(Error::QueryReturnedNoRows) => Ok(None),
            Ok(t) => Ok(Some(t)),
            Err(e) => Err(e)
        }
    }
}

 //  pub trait ActiveRusqlite {
//      fn create<T: ActiveRecord>(&self, record: &T) -> Result<(), Error>;
//      fn read<T: ActiveRecord>(&self) -> Result<Option<T>, Error>;
//      fn update<T: ActiveRecord>(&self, record: &mut T) -> Result<(), Error>;
//      fn delete<T: ActiveRecord>(&self) -> Result<(), Error>;

//      fn create_sub<R: ActiveRecord, T: ActiveRecord>(&self, path: &[String], record: &T) -> Result<(), Error>;
//      fn read_sub<R: ActiveRecord, T: ActiveRecord>(&self, path: &[String]) -> Result<Option<T>, Error>;
//      fn update_sub<R: ActiveRecord, T: ActiveRecord>(&self, path: &[String], record: &mut T) -> Result<(), Error>;
//      fn delete_sub<R: ActiveRecord>(&self, path: &[String]) -> Result<(), Error>;
//  }
//
//  impl ActiveRusqlite for Connection {
//      fn create<T: ActiveRecord>(&self, record: &T) -> Result<(), Error> {
//          _Location::create(self, &T::record_type(), &[T::name()], record.get_raw())
//      }
//      fn read<T: ActiveRecord>(&self) -> Result<Option<T>, Error>{
//          Ok(_Location::read(self, &T::record_type(), &[T::name()])?.map(|r| T::from_raw(r)))
//      }
//      fn update<T: ActiveRecord>(&self, record: &mut T) -> Result<(), Error> {
//          _Location::update(self, &T::record_type(), &[T::name()], record.get_record_mut())
//      }
//      fn delete<T: ActiveRecord>(&self) -> Result<(), Error> {
//          _Location::delete(self,  &T::record_type(), &[T::name()])
//      }

//      fn create_sub<R: ActiveRecord, T: ActiveRecord>(&self, path: &[String], record: &T) -> Result<(), Error> {
//          _Location::create(self, &R::record_type(), &[&[R::name()], path].concat(), record.get_raw())
//      }
//      fn read_sub<R: ActiveRecord, T: ActiveRecord>(&self, path: &[String]) -> Result<Option<T>, Error> {
//          Ok(_Location::read(self, &R::record_type(), &[&[R::name()], path].concat())?.map(|r| T::from_raw(r)))
//      }
//      fn update_sub<R: ActiveRecord, T: ActiveRecord>(&self, path: &[String], record: &mut T) -> Result<(), Error> {
//          _Location::update(self, &R::record_type(), &[&[R::name()], path].concat(), record.get_record_mut())
//      }
//      fn delete_sub<R: ActiveRecord>(&self, path: &[String]) -> Result<(), Error> {
//          _Location::delete(self,  &R::record_type(), &[&[R::name()], path].concat())
//      }
//  }

pub trait ActiveRusqlite: ActiveRecord {
    fn create(&self, rusqlite: &Connection) -> Result<(), Error>;
    fn read(rusqlite: &Connection) -> Result<Option<Self>, Error> where Self: Sized;
    fn update(&mut self, rusqlite: &Connection) -> Result<(), Error>;
    fn delete(rusqlite: &Connection) -> Result<(), Error>;

    fn create_sub<T: ActiveRecord>(rusqlite: &Connection, path: &[&str], record: &T) -> Result<(), Error>;
    fn read_sub<T: ActiveRecord>(rusqlite: &Connection, path: &[&str]) -> Result<Option<T>, Error>;
    fn update_sub<T: ActiveRecord>(rusqlite: &Connection,  path: &[&str], record: &mut T) -> Result<(), Error>;
    fn delete_sub(rusqlite: &Connection, path: &[&str]) -> Result<(), Error>;
}

impl<A: ActiveRecord> ActiveRusqlite for A {
     fn create(&self, rusqlite: &Connection) -> Result<(), Error> {
        _Location::create(rusqlite, &Self::record_type(), &[&Self::name()], self.get_raw())
     }
    fn read(rusqlite: &Connection) -> Result<Option<Self>, Error> {
        Ok(_Location::read(rusqlite, &Self::record_type(), &[&Self::name()])?.map(|r| Self::from_raw(r)))
    }
    fn update(&mut self, rusqlite: &Connection) -> Result<(), Error> {
        _Location::update(rusqlite, &Self::record_type(), &[&Self::name()], self.get_record_mut())
    }
    fn delete(rusqlite: &Connection) -> Result<(), Error> {
        _Location::delete(rusqlite, &Self::record_type(), &[&Self::name()])
    }

    fn create_sub<T: ActiveRecord>(rusqlite: &Connection, path: &[&str], record: &T) -> Result<(), Error> {

        _Location::create(rusqlite, &Self::record_type(), &[&[Self::name().as_str()], path].concat(), record.get_raw())
    }
    fn read_sub<T: ActiveRecord>(rusqlite: &Connection, path: &[&str]) -> Result<Option<T>, Error> {
        Ok(_Location::read(rusqlite, &Self::record_type(), &[&[Self::name().as_str()], path].concat())?.map(|r| T::from_raw(r)))
    }
    fn update_sub<T: ActiveRecord>(rusqlite: &Connection,  path: &[&str], record: &mut T) -> Result<(), Error> {
        _Location::update(rusqlite, &Self::record_type(), &[&[Self::name().as_str()], path].concat(), record.get_record_mut())
    }
    fn delete_sub(rusqlite: &Connection, path: &[&str]) -> Result<(), Error> {
        _Location::delete(rusqlite, &Self::record_type(), &[&[Self::name().as_str()], path].concat())
    }
}

trait _Location {
    fn create(&self, root: &RecordType, path: &[&str], raw: RawRecord) -> Result<(), Error>;
    fn read(&self, root: &RecordType, path: &[&str]) -> Result<Option<RawRecord>, Error>;
    fn update(&self, root: &RecordType, path: &[&str], record: RecordMut<'_>) -> Result<(), Error>;
    fn delete(&self, root: &RecordType, path: &[&str]) -> Result<(), Error>;

    fn create_table(&self, root: &RecordType, path: &[&str]) -> Result<(), Error>;
}

///All combinations of types that are relivent to sqlite
///Maps can have State or not
///Structs can have Self or not and if Self, State or Not
///Map->Struct with self has special form and so needs All varients of Map and Struct combinations
#[derive(Debug)]
enum SqliteType {
    Map,
    MapState,
    MapStructSelf(Option<(String, BTreeSet<String>)>),
    MapStructSelfState(Option<(String, BTreeSet<String>)>),
    MapStateStructSelf(Option<(String, BTreeSet<String>)>),
    MapStateStructSelfState(Option<(String, BTreeSet<String>)>),
    Struct(BTreeSet<String>),
    StructSelf(BTreeSet<String>),
    StructSelfState(BTreeSet<String>),
}

impl SqliteType {
    fn id(&self) -> Option<&str> {
        match self {
            Self::MapStructSelf(Some((id, _))) => Some(id),
            Self::MapStructSelfState(Some((id, _))) => Some(id),
            Self::MapStateStructSelf(Some((id, _))) => Some(id),
            Self::MapStateStructSelfState(Some((id, _))) => Some(id),
            Self::StructSelf(_) => Some("0"),
            Self::StructSelfState(_) => Some("0"),
            _ => None
        }
    }
    fn map(&self) -> Option<&BTreeSet<String>> {
        match self {
            Self::MapStructSelf(Some((_, map))) => Some(map),
            Self::MapStructSelfState(Some((_, map))) => Some(map),
            Self::MapStateStructSelf(Some((_, map))) => Some(map),
            Self::MapStateStructSelfState(Some((_, map))) => Some(map),
            Self::Struct(map) => Some(map),
            Self::StructSelf(map) => Some(map),
            Self::StructSelfState(map) => Some(map),
            _ => None
        }
    }

    fn has_state(&self) -> bool {
        matches!(self, Self::MapState | Self::MapStructSelfState(Some(_)) |
            Self::StructSelfState(_) | Self::MapStateStructSelf(None) |
            Self::MapStateStructSelfState(_)
        )
    }

    fn from<'a>(root: &'a RecordType, path: &'a [&'a str]) -> Result<(Self, String), Error> {
        let root_get = |path: &[&str]| root.get(path).map_err(|e| Error::InvalidPath(PathBuf::from(e)));
        let struct_path = || path.join("_").to_string();
        let map_path = || path.parent().join("_").to_string();
        Ok(match root_get(path)? {
            RecordType::Map(state, map) => {
                (match (state, &**map) {
                    (false, RecordType::Struct(Some(false), _)) => Self::MapStructSelf(None),
                    (false, RecordType::Struct(Some(true), _)) => Self::MapStructSelfState(None),
                    (true, RecordType::Struct(Some(false), _)) => Self::MapStateStructSelf(None),
                    (true, RecordType::Struct(Some(true), _)) => Self::MapStateStructSelfState(None),
                    (false, _) => Self::Map,
                    (true, _) => Self::MapState,
                }, struct_path())
            },
            RecordType::Struct(selfstate, map) => {
                let parent = if path.len() < 2 {None} else {match root_get(path.parent())? {
                    RecordType::Map(state, _) => Some(state), _ => None
                }};
                let map = map.keys().cloned().collect::<BTreeSet<String>>();
                match (parent, selfstate) {
                    (_, None) => (Self::Struct(map), path.join("_")),
                    (None, Some(false)) => (Self::StructSelf(map), struct_path()),
                    (None, Some(true)) => (Self::StructSelfState(map), struct_path()),
                    (Some(true), Some(false)) => (
                        Self::MapStateStructSelf(Some((path.last().unwrap().to_string(), map))), map_path() 
                    ),
                    (Some(true), Some(true)) => (
                        Self::MapStateStructSelfState(Some((path.last().unwrap().to_string(), map))), map_path() 
                    ),
                    (Some(false), Some(true)) => (
                        Self::MapStructSelfState(Some((path.last().unwrap().to_string(), map))), map_path() 
                    ),
                    (Some(false), Some(false)) => (
                        Self::MapStructSelf(Some((path.last().unwrap().to_string(), map))), map_path() 
                    ),
                }
            }
        })
    }
}

impl _Location for Connection {
    fn create_table(&self, root: &RecordType, path: &[&str]) -> Result<(), Error> {
        let (ty, table) = SqliteType::from(root, path)?;
        let cmd = match ty {
            SqliteType::Map => format!("
                CREATE TABLE IF NOT EXISTS \"{table}\"(id TEXT NOT NULL UNIQUE)
            "),
            SqliteType::MapState => format!("
                CREATE TABLE IF NOT EXISTS \"{table}\"(
                id TEXT NOT NULL UNIQUE,
                map_state TEXT
            )"),
            SqliteType::MapStructSelf(_) => format!("
                CREATE TABLE IF NOT EXISTS \"{table}\"(
                id TEXT NOT NULL UNIQUE,
                self TEXT NOT NULL
            )"),
            SqliteType::MapStructSelfState(_) => format!("
                CREATE TABLE IF NOT EXISTS \"{table}\"(
                id TEXT NOT NULL UNIQUE,
                self TEXT NOT NULL,
                state TEXT NOT NULL
            )"),
            SqliteType::MapStateStructSelf(_) => format!("
                CREATE TABLE IF NOT EXISTS \"{table}\"(
                id TEXT NOT NULL UNIQUE,
                map_state TEXT,
                self TEXT NOT NULL
            )"),
            SqliteType::MapStateStructSelfState(_) => format!("
                CREATE TABLE IF NOT EXISTS \"{table}\"(
                id TEXT NOT NULL UNIQUE,
                map_state TEXT,
                self TEXT NOT NULL,
                state TEXT NOT NULL
            )"),
            SqliteType::Struct(_) => {return Ok(());}
            SqliteType::StructSelf(_) => format!("
                CREATE TABLE IF NOT EXISTS \"{table}\"(
                id TEXT NOT NULL UNIQUE,
                self TEXT NOT NULL
            )"),
            SqliteType::StructSelfState(_) => format!("
                CREATE TABLE IF NOT EXISTS \"{table}\"(
                id TEXT NOT NULL UNIQUE,
                self TEXT NOT NULL,
                state TEXT NOT NULL
            )"),
        };
        self.execute(&cmd, [])?;
        Ok(())
    }

    //name room, path messages
    fn create(&self, root: &RecordType, path: &[&str], raw: RawRecord) -> Result<(), Error>{
        self.create_table(root, path)?;
        let (ty, table) = SqliteType::from(root, path)?;
        let map = match raw {
            RawRecord::Map(state, map) => {
                match ty {
                    SqliteType::Map => {
                        for k in map.keys() {
                            self.execute(&format!("
                                INSERT INTO \"{table}\"(id)
                                VALUES (?1)
                                ON CONFLICT DO NOTHING
                            ;"), [k])?;
                        }
                    },
                    SqliteType::MapState => {
                        self.execute(&format!("
                            INSERT INTO \"{table}\"(id, map_state)
                            VALUES ('__map_state__', ?1)
                            ON CONFLICT DO UPDATE SET
                                map_state=excluded.map_state
                        ;"), [state.unwrap()])?;

                        for k in map.keys() {
                            self.execute(&format!("
                                INSERT INTO \"{table}\"(id)
                                VALUES (?1)
                                ON CONFLICT DO NOTHING
                            ;"), [k])?;
                        }
                    },
                    SqliteType::MapStateStructSelf(None) => {
                        self.execute(&format!("
                            INSERT INTO \"{table}\"(id, map_state, self)
                            VALUES ('__map_state__', ?1, '')
                            ON CONFLICT DO UPDATE SET
                                map_state=excluded.map_state,
                                self=excluded.self
                        ;"), [state.unwrap()])?;
                    },
                    SqliteType::MapStateStructSelfState(None) => {
                        self.execute(&format!("
                            INSERT INTO \"{table}\"(id, map_state, self, state)
                            VALUES ('__map_state__', ?1, '', '')
                            ON CONFLICT DO UPDATE SET
                                map_state=excluded.map_state,
                                self=excluded.self,
                                state=excluded.state
                        ;"), [state.unwrap()])?;
                    },
                    SqliteType::MapStructSelfState(None) | SqliteType::MapStructSelf(None) => {
                        //Valid states where we do nothing
                    },
                    s => {panic!("Invalid State: {s:?}");},
                }
                map
            },
            RawRecord::Struct(Some((state, selfs)), map) => {
                match ty {
                    SqliteType::MapStructSelf(Some((id, _))) | SqliteType::MapStateStructSelf(Some((id, _))) => {
                        self.execute(&format!("
                            INSERT INTO \"{table}\"(id, self)
                            VALUES (?1, ?2)
                            ON CONFLICT DO UPDATE SET
                                self=excluded.self
                        ;"), [id, selfs])?;
                    },
                    SqliteType::MapStructSelfState(Some((id, _))) | SqliteType::MapStateStructSelfState(Some((id, _))) => {
                        self.execute(&format!("
                            INSERT INTO \"{table}\"(id, self, state)
                            VALUES (?1, ?2, ?3)
                            ON CONFLICT DO UPDATE SET
                                self=excluded.self,
                                state=excluded.state
                        ;"), [id, selfs, state.unwrap()])?;
                    },
                    SqliteType::StructSelf(_) => {
                        self.execute(&format!("
                            INSERT INTO \"{table}\"(id, self)
                            VALUES ('0', ?1)
                            ON CONFLICT DO UPDATE SET
                                self=excluded.self
                        ;"), [&selfs])?;
                    },
                    SqliteType::StructSelfState(_) => {
                        self.execute(&format!("
                            INSERT INTO \"{table}\"(id, self, state)
                            VALUES ('0', ?1, ?2)
                            ON CONFLICT DO UPDATE SET
                                self=excluded.self,
                                state=excluded.state
                        ;"), [&selfs, &state.unwrap()])?;
                    },
                    s => {panic!("Invalid State: {s:?}");},
                }
                map
            },
            RawRecord::Struct(None, map) if matches!(ty, SqliteType::Struct(_)) => map,
            s => {panic!("Invalid State: {s:?}");},
        };
        for (k, v) in map {
            _Location::create(self, root, &[path, &[&k]].concat(), v)?;
        }
        Ok(())
    }

    fn read(&self, root: &RecordType, path: &[&str]) -> Result<Option<RawRecord>, Error> {
        let (ty, table) = SqliteType::from(root, path)?;
        Ok(Some(match ty.map() {
            Some(map) => {
                let children = map.iter().flat_map(|k|
                    _Location::read(self, root, &[path, &[k]].concat()).transpose().map(|r| Ok((k.to_string(), r?)))
                ).collect::<Result<BTreeMap<String, RawRecord>, Error>>()?;

                let selfstate = match ty {
                    SqliteType::StructSelf(_) => {
                        if let Some(v) = self.query_row(&format!("
                            SELECT self FROM \"{table}\" WHERE id = '0'
                        "), [], |row| Ok(Some((None, row.get(0)?)))).optional()? {v} else {return Ok(None);}
                    },
                    SqliteType::StructSelfState(_) => {
                        if let Some(v) = self.query_row(&format!("
                            SELECT self, state FROM \"{table}\" WHERE id = '0'
                        "), [], |row| Ok(Some((Some(row.get(1)?), row.get(0)?)))).optional()? {v} else {return Ok(None);}
                    },
                    SqliteType::MapStructSelf(Some((id, _))) | SqliteType::MapStateStructSelf(Some((id, _))) => {
                        if let Some(v) = self.query_row(&format!("
                            SELECT self FROM \"{table}\" WHERE id = '{id}'
                        "), [], |row| Ok(Some((None, row.get(0)?)))).optional()? {v} else {return Ok(None);}
                    },
                    SqliteType::MapStructSelfState(Some((id, _))) | SqliteType::MapStateStructSelfState(Some((id, _))) => {
                        if let Some(v) = self.query_row(&format!("
                            SELECT self, state FROM \"{table}\" WHERE id = '{id}'
                        "), [], |row| Ok(Some((Some(row.get(1)?), row.get(0)?)))).optional()? {v} else {return Ok(None);}
                    },
                    SqliteType::Struct(_) => {
                        None
                    },
                    s => {panic!("Invalid State: {s:?}");},
                };

                RawRecord::Struct(selfstate, children)
            },
            None => {
                let map_state = if ty.has_state() {
                    if let Some(state) = self.query_row(&format!("
                        SELECT map_state FROM \"{table}\" WHERE id = '__map_state__'
                    "), [], |row| row.get(0)).optional()? 
                    {Some(state)} else {return Ok(None);}
                } else {None};

                let children = self.prepare(&format!("
                    SELECT id FROM \"{table}\"
                ")).optional()?.map(|mut q| Ok::<_, Error>(q.query([]).optional()?.map(|r| Ok::<_, Error>(r.map(|row|
                    row.get(0)
                ).collect::<Vec<String>>().optional()?.map(|c| c.into_iter().flat_map(|k|
                    _Location::read(self, root, &[path, &[&k]].concat()).transpose().map(|r| Ok((k, r?)))
                ).collect::<Result<BTreeMap<String, RawRecord>, Error>>()))))).transpose()?.flatten().transpose()?.flatten().transpose()?.unwrap_or_default();

                RawRecord::Map(map_state, children)
            }
        }))
    }

    fn update(&self, root: &RecordType, path: &[&str], record: RecordMut<'_>) -> Result<(), Error> {
        self.create_table(root, path)?;
        let (ty, table) = SqliteType::from(root, path)?;

        match record {
            RecordMut::Map(map) => {
                let map_state: Option<String> = if ty.has_state() {
                    self.query_row(&format!("
                        SELECT map_state FROM \"{table}\" WHERE id = '__map_state__'
                    "), [], |row| row.get(0)).optional()? 
                } else {None};

                let children = self.prepare(&format!("
                    SELECT id FROM \"{table}\"
                ")).optional()?.map(|mut q| Ok::<_, Error>(q.query([]).optional()?.map(|r| r.map(|row|
                    row.get(0)
                ).collect::<BTreeSet<String>>().optional()))).transpose()?.flatten().transpose()?.flatten().unwrap_or_default();
                let (set_remote, instructions) = map.state(map_state.as_deref(), &children);
                //If set_remote is true use map.get_state() even if it its true if map_state is
                //none use local state If remote state is none and local state
                let map_state = map_state.filter(|_| !set_remote).or_else(|| map.get_state());
                if set_remote && map_state.is_some() {
                    _Location::create(self, root, path, RawRecord::Map(map_state.clone(), BTreeMap::default()))?;
                }

                for (k, o) in instructions {
                    //println!("instructions: {:?} : {:?}", k, o);
                    let child_path = [path, &[&k]].concat();
                    match o {
                        Ordering::Greater => {
                            if let Some(child) = map.get_children_mut().get(&k) {
                                _Location::create(self, root, path,
                                    RawRecord::Map(map_state.clone(), BTreeMap::from([(k.to_string(), child.get_raw())]))
                                )?;
                            } else {
                                //If the mapping claims ours is greater but we don't have the child
                                //remove it from remote
                                _Location::delete(self, root, &child_path)?;
                            }
                        },
                        Ordering::Less => {
                            match _Location::read(self, root, &child_path)? {
                                Some(child) => {map.active_map_insert(k, child);},
                                None => {map.active_map_remove(k);}
                            }
                        },
                        Ordering::Equal => {
                            _Location::update(self, root, &child_path, map.get_children_mut().remove(&k).unwrap())?;
                        }
                    };
                }
            },
            RecordMut::Struct(st) => {
                let state: Option<Option<String>> = match ty {
                    SqliteType::StructSelfState(_) => {
                        Some(self.query_row(&format!("
                            SELECT state FROM \"{table}\" WHERE id = '0'
                        "), [], |row| row.get(0)).optional()?)
                    },
                    SqliteType::MapStructSelfState(Some((ref id, _))) | SqliteType::MapStateStructSelfState(Some((ref id, _))) => {
                        Some(self.query_row(&format!("
                            SELECT state FROM \"{table}\" WHERE id = '{id}'
                        "), [], |row| row.get(0)).optional()?)
                    },
                    ref ty => {
                        if let Some(id) = ty.id() {
                            self.query_row(&format!("
                                SELECT id FROM \"{table}\" WHERE id = '{id}'
                            "), [], |row| row.get(0)).optional()?.map(|_: Option<String>| None)
                        } else {None}
                    }
                };
                let deref_state = state.as_ref().map(|s| s.as_deref());
                let ordering = st.state(deref_state);
                //println!("instructions: {:?} : {:?}", ty, ordering);
                match ordering  {
                    Ordering::Greater => {
                        match ty {
                            SqliteType::MapStructSelf(Some((id, _))) | SqliteType::MapStateStructSelf(Some((id, _))) => {
                                self.execute(&format!("
                                    INSERT INTO \"{table}\"(id, self)
                                    VALUES (?1, ?2)
                                    ON CONFLICT DO UPDATE SET
                                        self=excluded.self
                                ;"), [id, st.get_self().unwrap()])?;
                            },
                            SqliteType::MapStructSelfState(Some((id, _))) | SqliteType::MapStateStructSelfState(Some((id, _))) => {
                                self.execute(&format!("
                                    INSERT INTO \"{table}\"(id, self, state)
                                    VALUES (?1, ?2, ?3)
                                    ON CONFLICT DO UPDATE SET
                                        self=excluded.self,
                                        state=excluded.state
                                ;"), [id, st.get_self().unwrap(), st.get_state().unwrap()])?;
                            },
                            SqliteType::StructSelf(_) => {
                                self.execute(&format!("
                                    INSERT INTO \"{table}\"(id, self)
                                    VALUES ('0', ?1)
                                    ON CONFLICT DO UPDATE SET
                                        self=excluded.self
                                ;"), [&st.get_self().unwrap()])?;
                            },
                            SqliteType::StructSelfState(_) => {
                                self.execute(&format!("
                                    INSERT INTO \"{table}\"(id, self, state)
                                    VALUES ('0', ?1, ?2)
                                    ON CONFLICT DO UPDATE SET
                                        self=excluded.self,
                                        state=excluded.state
                                ;"), [&st.get_self().unwrap(), &st.get_state().unwrap()])?;
                            },
                            s => {panic!("Invalid State: {s:?}");},
                        }
                    },
                    Ordering::Equal => {},
                    Ordering::Less => {
                        //unwrap the state because it must exists if Ordering is Less
                        if let Some(state) = state {st.set_state(&state.unwrap());}
                        let selfs = match ty {
                            SqliteType::MapStructSelfState(Some((id, _))) | SqliteType::MapStateStructSelfState(Some((id, _))) |
                            SqliteType::MapStructSelf(Some((id, _))) | SqliteType::MapStateStructSelf(Some((id, _))) => {
                                self.query_row(&format!("
                                    SELECT self FROM \"{table}\" WHERE id = '{id}'
                                "), [], |row| row.get(0))?
                            },
                            SqliteType::StructSelf(_) | SqliteType::StructSelfState(_) => {
                                self.query_row(&format!("
                                    SELECT self FROM \"{table}\" WHERE id = '0'
                                "), [], |row| row.get(0))?
                            },
                            s => {panic!("Invaild state: {s:?}");}
                        };
                        st.set_self(selfs);
                    }
                }

                for (k, v) in st.get_children_mut() {
                    _Location::update(self, root, &[path, &[&k]].concat(), v)?;
                }
            }
        }
        Ok(())
    }
    fn delete(&self, root: &RecordType, path: &[&str]) -> Result<(), Error> {
        let (ty, table) = SqliteType::from(root, path)?;

        let children = match ty.map() {
            Some(map) => map,
            None => {
                &self.prepare(&format!("
                    SELECT id FROM \"{table}\"
                ")).optional()?.map(|mut q| Ok::<_, Error>(q.query([]).optional()?.map(|r| Ok::<_, Error>(r.map(|row|
                    row.get(0)
                ).collect::<BTreeSet<String>>())))).transpose()?.flatten().transpose()?.transpose()?.unwrap_or_default()
            }
        };

        for k in children {
            _Location::delete(self, root, &[path, &[k]].concat())?;
        }

        let cmd = match ty {
            SqliteType::MapStructSelf(Some((id, _))) | SqliteType::MapStateStructSelf(Some((id, _))) |
            SqliteType::MapStructSelfState(Some((id, _))) | SqliteType::MapStateStructSelfState(Some((id, _))) => {
                format!("DELETE FROM \"{table}\" WHERE id = '{id}'")
            },
            _ => {
                format!("DROP TABLE IF EXISTS \"{table}\"")
            }
        };
        self.execute(&cmd, []).optional()?;

        Ok(())
    }
}
