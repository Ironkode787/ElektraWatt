// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Sets and keyed lists (doc 05 §3.2): containers that keep the canonical order themselves, so a value
//! built in memory is written sorted, and that reject unsorted or duplicate input on read.
//!
//! - A [`Set`] is sorted by the **compact JSON text** of its elements, byte-wise, without duplicates.
//!   For identifier strings that is byte order of the text except where JSON escapes or the closing quote
//!   intervene (`"ab!"` sorts before `"ab"`, because `!` is below `"`).
//! - A [`KeyedList`] is sorted by its elements' key in the key type's natural order (numbers numerically,
//!   identifiers byte-wise), without two elements of one key.

use std::borrow::Cow;
use std::cmp::Ordering;
use std::fmt;
use std::ops::Deref;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::SchemaError;
use crate::track::{current_path, push_token, raise};

/// The sort key of a set element: its compact JSON text.
fn json_key<T: Serialize>(item: &T) -> String {
    serde_json::to_string(item).unwrap_or_default()
}

/// The JSON Pointer of element `index` of the list being read.
fn element_path(index: usize) -> String {
    let mut path = current_path();
    push_token(&mut path, &index.to_string());
    path
}

// ---------------------------------------------------------------------------------------------------------
// Set

/// A set (doc 05 §3.2): its elements sorted by their compact JSON text, each once. Built from any
/// iterator (sorted and de-duplicated on the way in); read strictly, so unsorted input is
/// [`SchemaError::UnsortedList`] and a repeated element [`SchemaError::DuplicateKey`].
///
/// ```
/// use ew_mltext::Ident;
/// use ew_schema::Set;
///
/// let aliases: Set<Ident> = ["JE2416", "J-E-2416", "JE2416"].iter().map(|s| s.parse().unwrap()).collect();
/// assert_eq!(serde_json::to_string(&aliases).unwrap(), r#"["J-E-2416","JE2416"]"#);
/// assert!(serde_json::from_str::<Set<Ident>>(r#"["JE2416","J-E-2416"]"#).is_err());
/// ```
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Set<T>(Vec<T>);

impl<T> Default for Set<T> {
    fn default() -> Set<T> {
        Set(Vec::new())
    }
}

impl<T> Set<T> {
    /// The empty set.
    pub const fn new() -> Set<T> {
        Set(Vec::new())
    }

    /// The elements in canonical order.
    pub fn as_slice(&self) -> &[T] {
        &self.0
    }

    /// The elements in canonical order.
    pub fn into_vec(self) -> Vec<T> {
        self.0
    }

    /// Whether the set is empty (the default, omitted on write).
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<T: Serialize> Set<T> {
    fn position(&self, item: &T) -> Result<usize, usize> {
        let key = json_key(item);
        self.0.binary_search_by(|e| json_key(e).cmp(&key))
    }

    /// Adds `item`; `false` if an equal element (same JSON text) is already there.
    pub fn insert(&mut self, item: T) -> bool {
        match self.position(&item) {
            Ok(_) => false,
            Err(i) => {
                self.0.insert(i, item);
                true
            }
        }
    }

    /// Removes the element with the same JSON text as `item`; whether there was one.
    pub fn remove(&mut self, item: &T) -> bool {
        match self.position(item) {
            Ok(i) => {
                self.0.remove(i);
                true
            }
            Err(_) => false,
        }
    }

    /// Whether an element with the same JSON text as `item` is in the set.
    pub fn contains(&self, item: &T) -> bool {
        self.position(item).is_ok()
    }
}

impl<T: Serialize> FromIterator<T> for Set<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Set<T> {
        let mut keyed: Vec<(String, T)> = iter.into_iter().map(|t| (json_key(&t), t)).collect();
        keyed.sort_by(|a, b| a.0.cmp(&b.0));
        keyed.dedup_by(|a, b| a.0 == b.0);
        Set(keyed.into_iter().map(|(_, t)| t).collect())
    }
}

impl<T: Serialize> From<Vec<T>> for Set<T> {
    fn from(items: Vec<T>) -> Set<T> {
        items.into_iter().collect()
    }
}

impl<T> Deref for Set<T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        &self.0
    }
}

impl<'a, T> IntoIterator for &'a Set<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<T: Serialize> Serialize for Set<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de, T: Deserialize<'de> + Serialize> Deserialize<'de> for Set<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Set<T>, D::Error> {
        let items = Vec::<T>::deserialize(deserializer)?;
        let keys: Vec<String> = items.iter().map(json_key).collect();
        for (i, pair) in keys.windows(2).enumerate() {
            match pair[0].cmp(&pair[1]) {
                Ordering::Less => {}
                Ordering::Equal => {
                    return Err(raise(SchemaError::DuplicateKey {
                        path: element_path(i + 1),
                        key: pair[1].clone(),
                    }));
                }
                Ordering::Greater => {
                    return Err(raise(SchemaError::UnsortedList {
                        path: element_path(i + 1),
                    }));
                }
            }
        }
        Ok(Set(items))
    }
}

impl<T: JsonSchema> JsonSchema for Set<T> {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> Cow<'static, str> {
        format!("Set_of_{}", T::schema_name()).into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let items = generator.subschema_for::<T>();
        json_schema!({
            "type": "array",
            "items": items,
            "uniqueItems": true,
            "description": "A set: sorted by the compact JSON text of its elements, without duplicates (doc 05 §3.2)."
        })
    }
}

// ---------------------------------------------------------------------------------------------------------
// Keyed list

/// An element of a [`KeyedList`]: the member that keys it.
pub trait Keyed {
    /// The key type; its `Ord` is the list order.
    type Key: Ord + fmt::Display;

    /// The key of this element.
    fn key(&self) -> &Self::Key;
}

/// Implements [`Keyed`] for a struct keyed by one of its members.
macro_rules! keyed {
    ($ty:ty, $field:ident: $key:ty) => {
        impl $crate::set::Keyed for $ty {
            type Key = $key;

            fn key(&self) -> &$key {
                &self.$field
            }
        }
    };
}

pub(crate) use keyed;

/// A keyed list (doc 05 §3.2): elements sorted by their [`Keyed::key`], one element per key. Read
/// strictly: unsorted input is [`SchemaError::UnsortedList`], a repeated key [`SchemaError::DuplicateKey`].
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct KeyedList<T>(Vec<T>);

impl<T> Default for KeyedList<T> {
    fn default() -> KeyedList<T> {
        KeyedList(Vec::new())
    }
}

impl<T> KeyedList<T> {
    /// The empty list.
    pub const fn new() -> KeyedList<T> {
        KeyedList(Vec::new())
    }

    /// The elements in key order.
    pub fn as_slice(&self) -> &[T] {
        &self.0
    }

    /// The elements in key order.
    pub fn into_vec(self) -> Vec<T> {
        self.0
    }

    /// Whether the list is empty (the default, omitted on write).
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<T: Keyed> KeyedList<T> {
    /// The list of `items` in key order, or [`SchemaError::DuplicateKey`] if two share a key.
    pub fn from_vec(mut items: Vec<T>) -> Result<KeyedList<T>, SchemaError> {
        items.sort_by(|a, b| a.key().cmp(b.key()));
        if let Some(i) = items.windows(2).position(|w| w[0].key() == w[1].key()) {
            return Err(SchemaError::DuplicateKey {
                path: format!("/{}", i + 1),
                key: items[i + 1].key().to_string(),
            });
        }
        Ok(KeyedList(items))
    }

    /// The element of `key`.
    pub fn get(&self, key: &T::Key) -> Option<&T> {
        self.0
            .binary_search_by(|e| e.key().cmp(key))
            .ok()
            .map(|i| &self.0[i])
    }

    /// Inserts `item` at its key's place; returns the element it replaces.
    pub fn insert(&mut self, item: T) -> Option<T> {
        match self.0.binary_search_by(|e| e.key().cmp(item.key())) {
            Ok(i) => Some(std::mem::replace(&mut self.0[i], item)),
            Err(i) => {
                self.0.insert(i, item);
                None
            }
        }
    }

    /// Removes and returns the element of `key`.
    pub fn remove(&mut self, key: &T::Key) -> Option<T> {
        self.0
            .binary_search_by(|e| e.key().cmp(key))
            .ok()
            .map(|i| self.0.remove(i))
    }
}

impl<T> Deref for KeyedList<T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        &self.0
    }
}

impl<'a, T> IntoIterator for &'a KeyedList<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<T: Serialize> Serialize for KeyedList<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de, T: Deserialize<'de> + Keyed> Deserialize<'de> for KeyedList<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<KeyedList<T>, D::Error> {
        let items = Vec::<T>::deserialize(deserializer)?;
        for (i, pair) in items.windows(2).enumerate() {
            match pair[0].key().cmp(pair[1].key()) {
                Ordering::Less => {}
                Ordering::Equal => {
                    return Err(raise(SchemaError::DuplicateKey {
                        path: element_path(i + 1),
                        key: pair[1].key().to_string(),
                    }));
                }
                Ordering::Greater => {
                    return Err(raise(SchemaError::UnsortedList {
                        path: element_path(i + 1),
                    }));
                }
            }
        }
        Ok(KeyedList(items))
    }
}

impl<T: JsonSchema> JsonSchema for KeyedList<T> {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> Cow<'static, str> {
        format!("KeyedList_of_{}", T::schema_name()).into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let items = generator.subschema_for::<T>();
        json_schema!({
            "type": "array",
            "items": items,
            "description": "A keyed list: sorted by key, one element per key (doc 05 §3.2)."
        })
    }
}
