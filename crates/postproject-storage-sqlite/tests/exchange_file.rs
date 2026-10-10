//! Sealed files carry complete source facts without buffering a production.

use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};

use postproject_core::{
    MetadataProperty, MetadataValue, ObjectRef, PropertyId, RevisionContext, VocabularyId,
};
use postproject_protocol::{ClientId, Command, Extensions, Proposal, RequestId};
use postproject_storage_sqlite::{
    CheckpointLimits, ReplayLimits, SqliteProduction,
    exchange_file::{self, FileLimits, FileManifest, FileReader},
};

fn append(source: &mut SqliteProduction, value: u64) {
    let proposal = Proposal::new(
        source.exchange_scope().unwrap(),
        ClientId::new(),
        RequestId::new(),
        None,
        RevisionContext::default(),
        vec![Command::AppendMetadata {
            target: ObjectRef::Production(source.production().id()),
            property: property(),
            value: MetadataValue::u64(value),
        }],
        Extensions::default(),
    )
    .unwrap();
    source.submit_proposal(&proposal).unwrap();
}

fn property() -> MetadataProperty {
    MetadataProperty::new(
        VocabularyId::new("urn:sealed:opaque").unwrap(),
        PropertyId::new("exact").unwrap(),
    )
}

#[test]
fn checkpoint_then_record_matches_exact_source_state_and_duplicate_after_reopen() {
    let directory = tempfile::tempdir().unwrap();
    let mut source = SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
    append(&mut source, u64::MAX);
    let mut checkpoint = Vec::new();
    let manifest = exchange_file::write_checkpoint(&source, &mut checkpoint).unwrap();
    let path = directory.path().join("mirror ü.pproj");
    let mut mirror = exchange_file::import_checkpoint(
        &path,
        Cursor::new(&checkpoint),
        FileLimits::default(),
        CheckpointLimits::default(),
    )
    .unwrap();
    assert_eq!(mirror.exchange_head().unwrap(), manifest.head());
    append(&mut source, (1 << 53) + 1);
    let mut record = Vec::new();
    let manifest =
        exchange_file::write_record(source.record_reader(2).unwrap(), &mut record).unwrap();
    let reader = FileReader::open(Cursor::new(&record), FileLimits::default()).unwrap();
    assert_eq!(reader.manifest(), &FileManifest::Record(Box::new(manifest)));
    assert!(
        exchange_file::apply_record(
            &mut mirror,
            Cursor::new(&record),
            FileLimits::default(),
            ReplayLimits::default()
        )
        .unwrap()
    );
    assert_eq!(
        mirror.exchange_head().unwrap(),
        source.exchange_head().unwrap()
    );
    let target = ObjectRef::Production(source.production().id());
    assert_eq!(
        mirror.metadata_values(target, &property()).unwrap(),
        source.metadata_values(target, &property()).unwrap()
    );
    drop(mirror);
    let mut mirror = SqliteProduction::open(&path).unwrap();
    assert!(
        !exchange_file::apply_record(
            &mut mirror,
            Cursor::new(&record),
            FileLimits::default(),
            ReplayLimits::default()
        )
        .unwrap()
    );
    assert_eq!(
        mirror.changes_since(0, 10).unwrap(),
        source.changes_since(0, 10).unwrap()
    );
    assert!(
        exchange_file::apply_record(
            &mut mirror,
            Cursor::new(&checkpoint),
            FileLimits::default(),
            ReplayLimits::default()
        )
        .is_err()
    );
    assert!(
        exchange_file::import_checkpoint(
            directory.path().join("wrong-kind.pproj"),
            Cursor::new(&record),
            FileLimits::default(),
            CheckpointLimits::default()
        )
        .is_err()
    );
}

#[test]
fn invalid_seals_and_body_lengths_never_publish_a_destination() {
    let directory = tempfile::tempdir().unwrap();
    let source = SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
    let mut checkpoint = Vec::new();
    exchange_file::write_checkpoint(&source, &mut checkpoint).unwrap();
    let mut cases = vec![
        checkpoint[..7].to_vec(),
        checkpoint[..checkpoint.len() - 1].to_vec(),
    ];
    let mut unsupported = checkpoint.clone();
    unsupported[7] = 2;
    cases.push(unsupported);
    let mut excessive = checkpoint.clone();
    let trailer = excessive.len() - 8;
    excessive[trailer..].copy_from_slice(&u64::MAX.to_be_bytes());
    cases.push(excessive);
    let mut body = checkpoint.clone();
    body[8..16].copy_from_slice(&u64::MAX.to_be_bytes());
    cases.push(body);
    let mut appended = checkpoint.clone();
    appended.push(0);
    cases.push(appended);
    for (index, bytes) in cases.iter().enumerate() {
        let path = directory.path().join(format!("rejected-{index}.pproj"));
        assert!(
            exchange_file::import_checkpoint(
                &path,
                Cursor::new(bytes),
                FileLimits::default(),
                CheckpointLimits::default()
            )
            .is_err()
        );
        assert!(!path.exists());
    }
    let limits = FileLimits::new(
        u64::try_from(checkpoint.len()).unwrap() - 1,
        postproject_protocol::Limits::default(),
    )
    .unwrap();
    assert!(FileReader::open(Cursor::new(&checkpoint), limits).is_err());
    let mut body = checkpoint;
    body[8..16].copy_from_slice(&u64::MAX.to_be_bytes());
    let mut reader = FileReader::open(Cursor::new(body), FileLimits::default()).unwrap();
    assert!(reader.next_document().is_err());
    assert!(reader.next_document().is_err());
}

struct FailingWriter {
    bytes: Vec<u8>,
    remaining: usize,
}

impl Write for FailingWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            return Err(io::Error::other("injected output failure"));
        }
        let count = bytes.len().min(self.remaining);
        self.bytes.extend_from_slice(&bytes[..count]);
        self.remaining -= count;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn interrupted_output_is_unsealed_and_existing_destinations_are_preserved() {
    let directory = tempfile::tempdir().unwrap();
    let source = SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
    let mut writer = FailingWriter {
        bytes: vec![],
        remaining: 100,
    };
    assert!(exchange_file::write_checkpoint(&source, &mut writer).is_err());
    assert!(FileReader::open(Cursor::new(&writer.bytes), FileLimits::default()).is_err());
    let mut complete = Vec::new();
    exchange_file::write_checkpoint(&source, &mut complete).unwrap();
    let path = directory.path().join("existing.pproj");
    std::fs::write(&path, b"unrelated data").unwrap();
    assert!(
        exchange_file::import_checkpoint(
            &path,
            Cursor::new(&complete),
            FileLimits::default(),
            CheckpointLimits::default()
        )
        .is_err()
    );
    assert_eq!(std::fs::read(path).unwrap(), b"unrelated data");
}

#[test]
fn change_ranges_preserve_complete_prefixes_and_retry_the_same_file() {
    let directory = tempfile::tempdir().unwrap();
    let mut source = SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
    let from = source.exchange_head().unwrap();
    let mut checkpoint = Vec::new();
    exchange_file::write_checkpoint(&source, &mut checkpoint).unwrap();
    let path = directory.path().join("mirror.pproj");
    let mut mirror = exchange_file::import_checkpoint(
        &path,
        Cursor::new(checkpoint),
        FileLimits::default(),
        CheckpointLimits::default(),
    )
    .unwrap();
    append(&mut source, 1);
    let pinned = source.read_session().unwrap().into_read_only();
    let pinned_head = pinned.exchange_head().unwrap();
    append(&mut source, 2);
    assert_eq!(pinned.exchange_head().unwrap(), pinned_head);
    assert_eq!(pinned_head.sequence(), 1);
    drop(pinned);
    let mut bytes = Vec::new();
    let through = exchange_file::write_changes(&source, from, &mut bytes).unwrap();
    let failure_at = second_record_body(&bytes);
    let failing = FaultReader {
        inner: Cursor::new(&bytes),
        failure_at,
    };
    assert!(
        exchange_file::apply_changes(
            &mut mirror,
            failing,
            FileLimits::default(),
            ReplayLimits::default()
        )
        .is_err()
    );
    assert_eq!(mirror.exchange_head().unwrap().sequence(), 1);
    drop(mirror);
    let mut mirror = SqliteProduction::open(&path).unwrap();
    assert_eq!(
        exchange_file::apply_changes(
            &mut mirror,
            Cursor::new(&bytes),
            FileLimits::default(),
            ReplayLimits::default()
        )
        .unwrap(),
        through
    );
    assert_eq!(
        exchange_file::apply_changes(
            &mut mirror,
            Cursor::new(&bytes),
            FileLimits::default(),
            ReplayLimits::default()
        )
        .unwrap(),
        through
    );
    let target = ObjectRef::Production(source.production().id());
    assert_eq!(
        mirror.metadata_values(target, &property()).unwrap(),
        [MetadataValue::u64(1), MetadataValue::u64(2)]
    );
    let mut empty = Vec::new();
    assert_eq!(
        exchange_file::write_changes(&source, through, &mut empty).unwrap(),
        through
    );
    assert_eq!(
        exchange_file::apply_changes(
            &mut mirror,
            Cursor::new(&empty),
            FileLimits::default(),
            ReplayLimits::default()
        )
        .unwrap(),
        through
    );
    assert!(
        exchange_file::apply_changes(
            &mut source,
            Cursor::new(&empty),
            FileLimits::default(),
            ReplayLimits::default()
        )
        .is_err()
    );
    assert_eq!(
        mirror.changes_since(0, 10).unwrap(),
        source.changes_since(0, 10).unwrap()
    );
}

fn second_record_body(bytes: &[u8]) -> u64 {
    let mut offset = 8;
    let mut records = 0;
    loop {
        let length = usize::try_from(u64::from_be_bytes(
            bytes[offset..offset + 8].try_into().unwrap(),
        ))
        .unwrap();
        let document = postproject_protocol::Document::parse(
            &bytes[offset + 8..offset + 8 + length],
            postproject_protocol::Limits::default(),
        )
        .unwrap();
        if document.kind().ok() == Some("record.manifest") {
            records += 1;
            if records == 2 {
                return u64::try_from(offset + 8 + length).unwrap();
            }
        }
        offset += 8 + length;
        assert!(offset < bytes.len());
    }
}

struct FaultReader<'a> {
    inner: Cursor<&'a Vec<u8>>,
    failure_at: u64,
}

impl Read for FaultReader<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if self.inner.position() == self.failure_at {
            return Err(io::Error::other("injected range interruption"));
        }
        self.inner.read(bytes)
    }
}

impl Seek for FaultReader<'_> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.inner.seek(position)
    }
}

struct ConcurrentWriter {
    bytes: Vec<u8>,
    path: std::path::PathBuf,
    appended: bool,
}

impl Write for ConcurrentWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if !self.appended && self.bytes.len() >= 8 {
            append(&mut SqliteProduction::open(&self.path).unwrap(), 3);
            self.appended = true;
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn change_export_pins_its_head_before_a_concurrent_native_write() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("source.pproj");
    let mut source = SqliteProduction::create(&path, None).unwrap();
    let from = source.exchange_head().unwrap();
    let mut checkpoint = Vec::new();
    exchange_file::write_checkpoint(&source, &mut checkpoint).unwrap();
    append(&mut source, 1);
    append(&mut source, 2);
    let mut writer = ConcurrentWriter {
        bytes: vec![],
        path,
        appended: false,
    };
    let through = exchange_file::write_changes(&source, from, &mut writer).unwrap();
    assert!(writer.appended);
    assert_eq!(through.sequence(), 2);
    assert_eq!(source.exchange_head().unwrap().sequence(), 3);
    let mut mirror = exchange_file::import_checkpoint(
        directory.path().join("mirror.pproj"),
        Cursor::new(checkpoint),
        FileLimits::default(),
        CheckpointLimits::default(),
    )
    .unwrap();
    exchange_file::apply_changes(
        &mut mirror,
        Cursor::new(writer.bytes),
        FileLimits::default(),
        ReplayLimits::default(),
    )
    .unwrap();
    let target = ObjectRef::Production(source.production().id());
    assert_eq!(
        mirror.metadata_values(target, &property()).unwrap(),
        [MetadataValue::u64(1), MetadataValue::u64(2)]
    );
    let mut suffix = Vec::new();
    exchange_file::write_changes(&source, through, &mut suffix).unwrap();
    exchange_file::apply_changes(
        &mut mirror,
        Cursor::new(suffix),
        FileLimits::default(),
        ReplayLimits::default(),
    )
    .unwrap();
    assert_eq!(
        mirror.exchange_head().unwrap(),
        source.exchange_head().unwrap()
    );
    assert_eq!(
        mirror.metadata_values(target, &property()).unwrap(),
        source.metadata_values(target, &property()).unwrap()
    );
}
