//! The development local authority, checkpoint and passive catch-up recipe.

use std::fs::File;

use postproject_core::{
    MetadataProperty, MetadataValue, ObjectRef, PropertyId, RevisionContext, VocabularyId,
};
use postproject_protocol::{ClientId, Command, Extensions, Proposal, RequestId};
use postproject_storage_sqlite::{
    CheckpointLimits, ReplayLimits, SqliteProduction,
    exchange_file::{self, FileLimits},
};

#[test]
fn portable_exchange() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let authority_path = directory.path().join("authority.pproj");
    let mirror_path = directory.path().join("mirror.pproj");
    let checkpoint_path = directory.path().join("knowledge.ppxc");
    let changes_path = directory.path().join("changes.ppxd");
    let mut authority = SqliteProduction::create(&authority_path, Some("Documentary".into()))?;

    // [exchange-checkpoint]
    let mut output = File::create_new(&checkpoint_path)?;
    let checkpoint = exchange_file::write_checkpoint(&authority, &mut output)?;
    output.sync_all()?;
    drop(output);
    let mut mirror = exchange_file::import_checkpoint(
        &mirror_path,
        File::open(&checkpoint_path)?,
        FileLimits::default(),
        CheckpointLimits::default(),
    )?;
    assert_eq!(mirror.exchange_head()?, checkpoint.head());
    // [/exchange-checkpoint]

    // [exchange-submit]
    let target = ObjectRef::Production(authority.production().id());
    let property = MetadataProperty::new(
        VocabularyId::new("urn:studio:opaque")?,
        PropertyId::new("sample")?,
    );
    let proposal = Proposal::new(
        authority.exchange_scope()?,
        ClientId::new(),
        RequestId::new(),
        None,
        RevisionContext::default(),
        vec![Command::AppendMetadata {
            target,
            property: property.clone(),
            value: MetadataValue::u64((1 << 53) + 1),
        }],
        Extensions::default(),
    )?;
    let accepted = authority.submit_proposal(&proposal)?;
    drop(authority);
    let mut authority = SqliteProduction::open(&authority_path)?;
    let recovered =
        authority.submission_outcome(proposal.scope(), proposal.client(), proposal.request())?;
    assert_eq!(recovered.as_ref(), Some(&accepted));
    assert_eq!(authority.submit_proposal(&proposal)?, accepted);
    // [/exchange-submit]

    // [exchange-catch-up]
    let mut output = File::create_new(&changes_path)?;
    let through = exchange_file::write_changes(&authority, checkpoint.head(), &mut output)?;
    output.sync_all()?;
    drop(output);
    assert_eq!(
        exchange_file::apply_changes(
            &mut mirror,
            File::open(&changes_path)?,
            FileLimits::default(),
            ReplayLimits::default()
        )?,
        through
    );
    assert_eq!(
        mirror.metadata_values(target, &property)?,
        authority.metadata_values(target, &property)?
    );
    assert!(mirror.begin_transaction().is_err());
    // [/exchange-catch-up]
    assert_eq!(mirror.exchange_head()?, authority.exchange_head()?);
    assert_eq!(
        mirror.changes_since(0, 10)?,
        authority.changes_since(0, 10)?
    );
    Ok(())
}
