use serde::ser::{SerializeMap, SerializeSeq};
use serde::Serializer;
use std::collections::BTreeMap;
use std::fmt;
use std::fmt::Formatter;
use std::fs::File;

pub struct UploadValue {
    pub filename: String,
    pub content_type: Option<String>,
    pub content: File,
}

impl fmt::Debug for UploadValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Upload({})", self.filename)
    }
}

impl Clone for UploadValue {
    fn clone(&self) -> Self {
        Self {
            filename: self.filename.clone(),
            content_type: self.content_type.clone(),
            content: self.content.try_clone().unwrap(),
        }
    }
}

/// Represents a GraphQL value
#[derive(Clone, Debug)]
#[allow(missing_docs)]
pub enum Value {
    Null,
    Variable(String),
    Number(serde_json::Number),
    String(String),
    Boolean(bool),
    Enum(String),
    List(Vec<Value>),
    Object(BTreeMap<String, Value>),
    Upload(UploadValue),
}

impl serde::Serialize for Value {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        match self {
            Value::Null => serializer.serialize_none(),
            Value::Variable(variable) => serializer.serialize_str(&format!("${}", variable)),
            Value::Number(value) => value.serialize(serializer),
            Value::String(value) => serializer.serialize_str(value),
            Value::Boolean(value) => serializer.serialize_bool(*value),
            Value::Enum(value) => serializer.serialize_str(value),
            Value::List(value) => {
                let mut seq = serializer.serialize_seq(Some(value.len()))?;
                for item in value {
                    seq.serialize_element(item)?;
                }
                seq.end()
            }
            Value::Object(value) => {
                let mut map = serializer.serialize_map(Some(value.len()))?;
                for (key, value) in value {
                    map.serialize_entry(key, value)?;
                }
                map.end()
            }
            Value::Upload(_) => serializer.serialize_none(),
        }
    }
}

impl Default for Value {
    fn default() -> Self {
        Value::Null
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        use Value::*;

        match (self, other) {
            (Variable(a), Variable(b)) => a.eq(b),
            (Number(a), Number(b)) => a.eq(b),
            (String(a), String(b)) => a.eq(b),
            (Boolean(a), Boolean(b)) => a.eq(b),
            (Null, Null) => true,
            (Enum(a), Enum(b)) => a.eq(b),
            (List(a), List(b)) => {
                if a.len() != b.len() {
                    return false;
                }
                for i in 0..a.len() {
                    if !a[i].eq(&b[i]) {
                        return false;
                    }
                }
                true
            }
            (Object(a), Object(b)) => {
                if a.len() != b.len() {
                    return false;
                }
                for (key, a_value) in a.iter() {
                    if let Some(b_value) = b.get(key) {
                        if !a_value.eq(b_value) {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }
                true
            }
            (Upload(a), Upload(b)) => a.filename == b.filename,
            _ => false,
        }
    }
}

fn write_quoted(s: &str, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "\"")?;
    for c in s.chars() {
        match c {
            '\r' => write!(f, "\r")?,
            '\n' => writeln!(f)?,
            '\t' => write!(f, "\t")?,
            '"' => write!(f, "\"")?,
            '\\' => write!(f, "\\")?,
            '\u{0020}'..='\u{FFFF}' => write!(f, "{}", c)?,
            _ => write!(f, "\\u{:04}", c as u32).unwrap(),
        }
    }
    write!(f, "\"")
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Variable(name) => write!(f, "${}", name),
            Value::Number(num) => write!(f, "{}", *num),
            Value::String(ref val) => write_quoted(val, f),
            Value::Boolean(true) => write!(f, "true"),
            Value::Boolean(false) => write!(f, "false"),
            Value::Null => write!(f, "null"),
            Value::Enum(ref name) => write!(f, "{}", name),
            Value::List(ref items) => {
                write!(f, "[")?;
                if !items.is_empty() {
                    write!(f, "{}", items[0])?;
                    for item in &items[1..] {
                        write!(f, ", ")?;
                        write!(f, "{}", item)?;
                    }
                }
                write!(f, "]")
            }
            Value::Object(items) => {
                write!(f, "{{")?;
                let mut first = true;
                for (name, value) in items {
                    if first {
                        first = false;
                    } else {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", name)?;
                    write!(f, ": ")?;
                    write!(f, "{}", value)?;
                }
                write!(f, "}}")
            }
            Value::Upload(_) => write!(f, "null"),
        }
    }
}

impl From<Value> for serde_json::Value {
    fn from(value: Value) -> Self {
        match value {
            Value::Null => serde_json::Value::Null,
            Value::Variable(name) => name.into(),
            Value::Number(n) => serde_json::Value::Number(n),
            Value::String(s) => s.into(),
            Value::Boolean(v) => v.into(),
            Value::Enum(e) => e.into(),
            Value::List(values) => values
                .into_iter()
                .map(Into::into)
                .collect::<Vec<serde_json::Value>>()
                .into(),
            Value::Object(obj) => serde_json::Value::Object(
                obj.into_iter()
                    .map(|(name, value)| (name, value.into()))
                    .collect(),
            ),
            Value::Upload(_) => serde_json::Value::Null,
        }
    }
}

impl From<serde_json::Value> for Value {
    fn from(value: serde_json::Value) -> Self {
        match value {
            serde_json::Value::Null => Value::Null,
            serde_json::Value::Bool(n) => Value::Boolean(n),
            serde_json::Value::Number(n) => Value::Number(n),
            serde_json::Value::String(s) => Value::String(s),
            serde_json::Value::Array(ls) => Value::List(ls.into_iter().map(Into::into).collect()),
            serde_json::Value::Object(obj) => Value::Object(
                obj.into_iter()
                    .map(|(name, value)| (name, value.into()))
                    .collect(),
            ),
        }
    }
}
