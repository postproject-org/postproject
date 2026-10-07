//! Finite JSON input before deserialization or production access.

use std::{fs::File, io::Read, marker::PhantomData, path::Path};

use anyhow::{Context, Result};
use serde::{
    Deserialize, Deserializer,
    de::{DeserializeOwned, SeqAccess, Visitor},
};

const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;

pub(crate) fn read<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let file = File::open(path).with_context(|| format!("open JSON input {}", path.display()))?;
    let encoded = read_bounded(file, MAX_FILE_BYTES)
        .with_context(|| format!("read JSON input {}", path.display()))?;
    serde_json::from_slice(&encoded).map_err(|error| {
        postproject_core::Error::new(
            postproject_core::ErrorKind::InvalidArgument,
            format!("invalid JSON input: {error}"),
        )
        .into()
    })
}

fn read_bounded(reader: impl Read, limit: u64) -> Result<Vec<u8>> {
    let mut encoded = Vec::new();
    reader.take(limit + 1).read_to_end(&mut encoded)?;
    if u64::try_from(encoded.len())? > limit {
        return Err(crate::errors::invalid("JSON input exceeds 64 MiB").into());
    }
    Ok(encoded)
}

pub(crate) fn collection<'de, D, T>(deserializer: D) -> std::result::Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    limited(
        deserializer,
        postproject_core::MAX_METADATA_COLLECTION_ITEMS,
    )
}

pub(crate) fn large_collection<'de, D, T>(deserializer: D) -> std::result::Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    limited(deserializer, postproject_core::MAX_CONTENT_MEMBERS)
}

fn limited<'de, D, T>(deserializer: D, limit: usize) -> std::result::Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct Items<T> {
        limit: usize,
        marker: PhantomData<T>,
    }
    impl<'de, T: Deserialize<'de>> Visitor<'de> for Items<T> {
        type Value = Vec<T>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(formatter, "an array of at most {} items", self.limit)
        }

        fn visit_seq<A: SeqAccess<'de>>(
            self,
            mut sequence: A,
        ) -> std::result::Result<Self::Value, A::Error> {
            let mut items = Vec::new();
            while items.len() < self.limit {
                let Some(item) = sequence.next_element()? else {
                    return Ok(items);
                };
                items.push(item);
            }
            if sequence.next_element::<serde::de::IgnoredAny>()?.is_some() {
                return Err(serde::de::Error::custom(format!(
                    "array exceeds {} items",
                    self.limit
                )));
            }
            Ok(items)
        }
    }
    deserializer.deserialize_seq(Items {
        limit,
        marker: PhantomData,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_reader_stops_after_one_lookahead_byte() {
        let mut reader = std::io::Cursor::new(vec![b' '; 1024]);
        assert!(read_bounded(&mut reader, 16).is_err());
        assert_eq!(reader.position(), 17);
        assert_eq!(read_bounded(std::io::Cursor::new(b"{}"), 2).unwrap(), b"{}");
    }

    #[test]
    fn collection_limit_rejects_extra_items_without_decoding_them() {
        let mut decoder = serde_json::Deserializer::from_str("[1,2,{\"unrecognized\":true}]");
        let error = limited::<_, u32>(&mut decoder, 2).unwrap_err();
        assert!(error.to_string().contains("array exceeds 2 items"));
        let mut decoder = serde_json::Deserializer::from_str("[1,2]");
        assert_eq!(limited::<_, u32>(&mut decoder, 2).unwrap(), [1, 2]);
    }
}
