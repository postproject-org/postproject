use std::{cell::Cell, fmt};

use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Value};

use super::Limits;
use crate::FailureKind;

pub(super) struct State {
    limits: Limits,
    nodes: Cell<usize>,
    failure: Cell<FailureKind>,
}

impl State {
    pub(super) const fn new(limits: Limits) -> Self {
        Self {
            limits,
            nodes: Cell::new(0),
            failure: Cell::new(FailureKind::Malformed),
        }
    }

    pub(super) fn failure(&self) -> FailureKind {
        self.failure.get()
    }

    fn exceeded<E: de::Error>(&self) -> E {
        self.failure.set(FailureKind::LimitExceeded);
        E::custom("JSON resource limit")
    }
}

pub(super) struct Seed<'a> {
    state: &'a State,
    depth: usize,
}

impl<'a> Seed<'a> {
    pub(super) const fn new(state: &'a State) -> Self {
        Self { state, depth: 0 }
    }

    const fn child(&self) -> Self {
        Self {
            state: self.state,
            depth: self.depth + 1,
        }
    }

    fn container<E: de::Error>(&self) -> std::result::Result<(), E> {
        if self.depth >= self.state.limits.depth {
            return Err(self.state.exceeded());
        }
        Ok(())
    }
}

impl<'de> DeserializeSeed<'de> for Seed<'_> {
    type Value = Value;

    fn deserialize<D: de::Deserializer<'de>>(
        self,
        decoder: D,
    ) -> std::result::Result<Value, D::Error> {
        let nodes = self.state.nodes.get();
        if nodes == self.state.limits.nodes {
            return Err(self.state.exceeded());
        }
        self.state.nodes.set(nodes + 1);
        decoder.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Seed<'_> {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("strict JSON with exact integers encoded as strings")
    }

    fn visit_unit<E: de::Error>(self) -> std::result::Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> std::result::Result<Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> std::result::Result<Value, E> {
        Ok(Value::String(value.to_owned()))
    }

    fn visit_string<E: de::Error>(self, value: String) -> std::result::Result<Value, E> {
        Ok(Value::String(value))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> std::result::Result<Value, A::Error> {
        self.container()?;
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element_seed(self.child())? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> std::result::Result<Value, A::Error> {
        self.container()?;
        let mut values = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(de::Error::custom("duplicate JSON key"));
            }
            values.insert(key, map.next_value_seed(self.child())?);
        }
        Ok(Value::Object(values))
    }
}
