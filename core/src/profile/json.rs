//! A JSON document that keeps the order of object keys as written. `serde_json::Value` sorts
//! them (the crate is built without `preserve_order`), which would reorder a hand-made
//! profile on every save from the editor.

use std::fmt;

use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::ser::{Serialize, SerializeMap, SerializeSeq, Serializer};
use serde_json::{Map, Number, Value};

/// A JSON value whose objects keep their key order.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Json {
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    pub(crate) fn object() -> Json {
        Json::Object(Vec::new())
    }

    pub(crate) fn text(text: &str) -> Json {
        Json::String(text.to_owned())
    }

    pub(crate) fn texts(texts: &[String]) -> Json {
        Json::Array(texts.iter().map(|t| Json::text(t)).collect())
    }

    pub(crate) fn number(value: u32) -> Json {
        Json::Number(Number::from(value))
    }

    pub(crate) fn is_object(&self) -> bool {
        matches!(self, Json::Object(_))
    }

    /// The value of `key` in an object.
    pub(crate) fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub(crate) fn get_mut(&mut self, key: &str) -> Option<&mut Json> {
        match self {
            Json::Object(entries) => entries.iter_mut().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Sets `key` of an object: in its place if it exists, else at the end. A value that is
    /// no object becomes an empty object first.
    pub(crate) fn set(&mut self, key: &str, value: Json) {
        if !self.is_object() {
            *self = Json::object();
        }
        if let Json::Object(entries) = self {
            match entries.iter_mut().find(|(k, _)| k == key) {
                Some((_, slot)) => *slot = value,
                None => entries.push((key.to_owned(), value)),
            }
        }
    }

    /// Sets `key` of an object: in its place if it exists, else before the first of `before`
    /// that is there, else at the end.
    pub(crate) fn insert_before(&mut self, key: &str, value: Json, before: &[&str]) {
        if self.get(key).is_some() {
            self.set(key, value);
            return;
        }
        if let Json::Object(entries) = self {
            let at = entries
                .iter()
                .position(|(k, _)| before.contains(&k.as_str()))
                .unwrap_or(entries.len());
            entries.insert(at, (key.to_owned(), value));
        } else {
            self.set(key, value);
        }
    }

    /// Removes `key` of an object; `true` if it was there.
    pub(crate) fn remove(&mut self, key: &str) -> bool {
        match self {
            Json::Object(entries) => {
                let before = entries.len();
                entries.retain(|(k, _)| k != key);
                entries.len() != before
            }
            _ => false,
        }
    }

    /// The object at `key`, created at the end if missing (or replacing a non-object).
    pub(crate) fn object_mut(&mut self, key: &str) -> &mut Json {
        if !self.get(key).is_some_and(Json::is_object) {
            self.set(key, Json::object());
        }
        self.get_mut(key).expect("just set")
    }

    pub(crate) fn as_str(&self) -> Option<&str> {
        match self {
            Json::String(text) => Some(text),
            _ => None,
        }
    }

    pub(crate) fn as_array(&self) -> Option<&[Json]> {
        match self {
            Json::Array(items) => Some(items),
            _ => None,
        }
    }

    /// The same value for the engine (which reads `serde_json::Value`).
    pub(crate) fn to_value(&self) -> Value {
        match self {
            Json::Null => Value::Null,
            Json::Bool(b) => Value::Bool(*b),
            Json::Number(n) => Value::Number(n.clone()),
            Json::String(s) => Value::String(s.clone()),
            Json::Array(items) => Value::Array(items.iter().map(Json::to_value).collect()),
            Json::Object(entries) => {
                let mut map = Map::new();
                for (key, value) in entries {
                    map.insert(key.clone(), value.to_value());
                }
                Value::Object(map)
            }
        }
    }

    /// Pretty text with two spaces, the way the app writes the profile file.
    pub(crate) fn to_pretty(&self) -> String {
        let mut text = serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_owned());
        text.push('\n');
        text
    }
}

impl Serialize for Json {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Json::Null => serializer.serialize_unit(),
            Json::Bool(b) => serializer.serialize_bool(*b),
            Json::Number(n) => n.serialize(serializer),
            Json::String(s) => serializer.serialize_str(s),
            Json::Array(items) => {
                let mut seq = serializer.serialize_seq(Some(items.len()))?;
                for item in items {
                    seq.serialize_element(item)?;
                }
                seq.end()
            }
            Json::Object(entries) => {
                let mut map = serializer.serialize_map(Some(entries.len()))?;
                for (key, value) in entries {
                    map.serialize_entry(key, value)?;
                }
                map.end()
            }
        }
    }
}

struct JsonVisitor;

impl<'de> Visitor<'de> for JsonVisitor {
    type Value = Json;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("any JSON value")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Json, E> {
        Ok(Json::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Json, E> {
        Ok(Json::Number(value.into()))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Json, E> {
        Ok(Json::Number(value.into()))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Json, E> {
        Number::from_f64(value)
            .map(Json::Number)
            .ok_or_else(|| E::custom("not a finite number"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Json, E> {
        Ok(Json::String(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> Result<Json, E> {
        Ok(Json::String(value))
    }

    fn visit_unit<E>(self) -> Result<Json, E> {
        Ok(Json::Null)
    }

    fn visit_none<E>(self) -> Result<Json, E> {
        Ok(Json::Null)
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Json, D::Error> {
        Json::deserialize(deserializer)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Json, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element()? {
            items.push(item);
        }
        Ok(Json::Array(items))
    }

    /// A key written twice keeps its first place and its last value (like `serde_json`).
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Json, A::Error> {
        let mut entries: Vec<(String, Json)> = Vec::new();
        while let Some((key, value)) = map.next_entry::<String, Json>()? {
            match entries.iter_mut().find(|(k, _)| *k == key) {
                Some((_, slot)) => *slot = value,
                None => entries.push((key, value)),
            }
        }
        Ok(Json::Object(entries))
    }
}

impl<'de> Deserialize<'de> for Json {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Json, D::Error> {
        deserializer.deserialize_any(JsonVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_keep_their_order_through_a_round_trip() {
        let text = "{\"zeta\": 1, \"alpha\": {\"z\": [1, 2.5, \"x\"], \"a\": null}, \"mid\": true}";
        let doc: Json = serde_json::from_str(text).unwrap();
        let out = doc.to_pretty();
        let zeta = out.find("zeta").unwrap();
        let alpha = out.find("alpha").unwrap();
        let mid = out.find("mid").unwrap();
        assert!(zeta < alpha && alpha < mid, "{out}");
        assert!(
            out.find("\"z\"").unwrap() < out.find("\"a\"").unwrap(),
            "{out}"
        );
        assert!(out.contains("2.5"), "{out}");
        let again: Json = serde_json::from_str(&out).unwrap();
        assert_eq!(again, doc);
        assert_eq!(doc.to_value(), serde_json::from_str::<Value>(text).unwrap());
    }

    #[test]
    fn set_keeps_the_place_and_remove_drops_the_key() {
        let mut doc: Json = serde_json::from_str("{\"b\": 1, \"a\": 2}").unwrap();
        doc.set("b", Json::text("x"));
        doc.set("c", Json::Bool(true));
        assert!(doc.remove("a"));
        assert!(!doc.remove("a"));
        assert_eq!(
            serde_json::to_string(&doc).unwrap(),
            "{\"b\":\"x\",\"c\":true}"
        );
        assert!(doc.object_mut("d").is_object());
        assert_eq!(
            serde_json::to_string(&doc).unwrap(),
            "{\"b\":\"x\",\"c\":true,\"d\":{}}"
        );
        doc.insert_before("e", Json::number(1), &["c", "d"]);
        doc.insert_before("b", Json::number(2), &["c"]);
        doc.insert_before("f", Json::number(3), &["z"]);
        assert_eq!(
            serde_json::to_string(&doc).unwrap(),
            "{\"b\":2,\"e\":1,\"c\":true,\"d\":{},\"f\":3}"
        );
    }

    #[test]
    fn a_duplicate_key_keeps_its_first_place_and_last_value() {
        let doc: Json = serde_json::from_str("{\"a\": 1, \"b\": 2, \"a\": 3}").unwrap();
        assert_eq!(serde_json::to_string(&doc).unwrap(), "{\"a\":3,\"b\":2}");
    }
}
