use std::{path::Path, time::Instant};

use postproject_core::{
    MAX_METADATA_BINARY_BYTES, MAX_METADATA_NESTING_DEPTH, MetadataProperty, MetadataValue,
    ObjectRef, PropertyId, RevisionContext, VocabularyId,
};
use postproject_protocol::{ClientId, Command, Extensions, OutcomeStatus, Proposal, RequestId};
use postproject_storage_sqlite::SqliteProduction;

fn property(name: &str) -> MetadataProperty {
    MetadataProperty::new(
        VocabularyId::new("urn:benchmark:exchange").unwrap(),
        PropertyId::new(name).unwrap(),
    )
}

fn nested() -> MetadataValue {
    let mut value = MetadataValue::u64(u64::MAX);
    for _ in 1..MAX_METADATA_NESTING_DEPTH {
        value = MetadataValue::list(vec![value]).unwrap();
    }
    assert_eq!(value.nesting_depth(), MAX_METADATA_NESTING_DEPTH);
    value
}

pub(super) fn large(path: &Path) {
    let mut production = SqliteProduction::create(path, Some("Large metadata".into())).unwrap();
    let target = ObjectRef::Production(production.production().id());
    let binary = MetadataValue::bytes(vec![255; MAX_METADATA_BINARY_BYTES]).unwrap();
    let mut edit = production.begin_transaction().unwrap();
    edit.add_metadata_value(target, &property("binary"), &binary)
        .unwrap();
    edit.add_metadata_value(target, &property("nested"), &nested())
        .unwrap();
    edit.commit().unwrap();
    println!("binary_bytes\t{MAX_METADATA_BINARY_BYTES}");
    println!("metadata_depth\t{MAX_METADATA_NESTING_DEPTH}");
}

pub(super) fn verify_large(path: &Path) {
    let production = SqliteProduction::open(path).unwrap();
    let target = ObjectRef::Production(production.production().id());
    assert_eq!(
        production
            .metadata_values(target, &property("binary"))
            .unwrap(),
        [MetadataValue::bytes(vec![255; MAX_METADATA_BINARY_BYTES]).unwrap()]
    );
    assert_eq!(
        production
            .metadata_values(target, &property("nested"))
            .unwrap(),
        [nested()]
    );
    println!("binary_bytes_verified\t{MAX_METADATA_BINARY_BYTES}");
}

pub(super) fn outcomes(path: &Path) {
    drop(SqliteProduction::create(path, Some("Outcome growth".into())).unwrap());
    println!("empty_store_bytes\t{}", path.metadata().unwrap().len());
    let client = ClientId::new();
    for phase in ["accepted", "no_op", "rejected"] {
        let mut production = SqliteProduction::open(path).unwrap();
        let target = ObjectRef::Production(production.production().id());
        let started = Instant::now();
        for index in 0..1000 {
            let commands = match phase {
                "accepted" => vec![Command::AppendMetadata {
                    target,
                    property: property("value"),
                    value: MetadataValue::i64(index),
                }],
                "no_op" => vec![],
                "rejected" => vec![Command::RemoveMetadata {
                    target,
                    property: property("value"),
                }],
                _ => unreachable!(),
            };
            let proposal = Proposal::new(
                production.exchange_scope().unwrap(),
                client,
                RequestId::new(),
                None,
                RevisionContext::default(),
                commands,
                Extensions::default(),
            )
            .unwrap();
            let outcome = production.submit_proposal(&proposal).unwrap();
            match (phase, outcome.status()) {
                ("accepted", OutcomeStatus::Accepted(receipt)) => {
                    assert!(receipt.revision().is_some());
                }
                ("no_op", OutcomeStatus::Accepted(receipt)) => {
                    assert!(receipt.revision().is_none());
                }
                ("rejected", OutcomeStatus::Rejected(_)) => {}
                _ => panic!("unexpected outcome"),
            }
            assert_eq!(
                production
                    .submission_outcome(proposal.scope(), client, proposal.request())
                    .unwrap(),
                Some(outcome)
            );
        }
        println!("{phase}_1000_elapsed_ns\t{}", started.elapsed().as_nanos());
        assert_eq!(production.exchange_head().unwrap().sequence(), 1000);
        drop(production);
        println!("{phase}_store_bytes\t{}", path.metadata().unwrap().len());
    }
}
