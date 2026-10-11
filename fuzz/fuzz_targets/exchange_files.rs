#![no_main]

use std::io::Cursor;

use libfuzzer_sys::fuzz_target;
use postproject_protocol::{CheckpointChunk, CheckpointManifest, Document, Limits, StoreRole};
use postproject_storage_sqlite::{CheckpointLimits, ReplayLimits, SqliteProduction, exchange_file};

const INPUT_BYTES: usize = 2 * 1024 * 1024;

fn checkpoint_limits() -> CheckpointLimits {
    CheckpointLimits::new(
        INPUT_BYTES as u64,
        32 * 1024 * 1024,
        10_000,
        Limits::default(),
    )
    .unwrap()
}

fn file_limits() -> exchange_file::FileLimits {
    exchange_file::FileLimits::new(INPUT_BYTES as u64, Limits::default()).unwrap()
}

fn baseline(path: &std::path::Path, genesis: bool) -> SqliteProduction {
    // Fixed independently authored media facts give record mutations real
    // existing objects. The harness does not produce golden expectations.
    let encoded = if genesis {
        include_str!("../../tests/fixtures/exchange/checkpoints/genesis.json")
    } else {
        include_str!("../../tests/fixtures/exchange/checkpoints/media.json")
    };
    let fixture: serde_json::Value = serde_json::from_str(encoded).unwrap();
    let document = |value: &serde_json::Value| {
        Document::parse(&serde_json::to_vec(value).unwrap(), Limits::default()).unwrap()
    };
    let manifest = CheckpointManifest::from_document(&document(&fixture["manifest"])).unwrap();
    let chunks = fixture["chunks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| Ok(CheckpointChunk::from_document(&document(value)).unwrap()));
    SqliteProduction::import_checkpoint(path, &manifest, chunks, checkpoint_limits()).unwrap()
}

fuzz_target!(|input: &[u8]| {
    let Some((&mode, data)) = input.split_first() else {
        return;
    };
    if data.len() > INPUT_BYTES {
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("mirror.pproj");
    let result = if mode % 2 == 0 {
        let result = exchange_file::import_checkpoint(
            &path,
            Cursor::new(data),
            file_limits(),
            checkpoint_limits(),
        );
        if result.is_err() {
            assert!(!path.exists(), "failed import published a destination");
        }
        result
    } else {
        let mut mirror = baseline(&path, mode & 2 != 0);
        let before = mirror.exchange_head().unwrap();
        let applied = exchange_file::apply_record(
            &mut mirror,
            Cursor::new(data),
            file_limits(),
            ReplayLimits::new(INPUT_BYTES as u64, Limits::default()).unwrap(),
        );
        if applied.is_err() {
            assert_eq!(mirror.exchange_head().unwrap(), before);
        }
        Ok(mirror)
    };
    if let Ok(mirror) = result {
        let head = mirror.exchange_head().unwrap();
        exchange_file::write_checkpoint(&mirror, Vec::new()).unwrap();
        drop(mirror);
        let mut reopened = SqliteProduction::open(path).unwrap();
        assert_eq!(reopened.exchange_head().unwrap(), head);
        assert_eq!(reopened.exchange_role(), StoreRole::PassiveMirror);
        assert_eq!(
            reopened.begin_transaction().err().unwrap().kind(),
            postproject_core::ErrorKind::Unsupported
        );
    }
});
