//! Runs the produced-output task listing against real temporary files.
use postproject_core::{
    ActivityKind, MetadataProperty, MetadataValue, ObjectRef, PropertyId, RepresentationKind,
    Result, ToolIdentity, VocabularyId,
};
use postproject_media::{MediaSource, OutputRecipe, OutputSource, prepare_outputs};
use postproject_storage_sqlite::SqliteProduction;

#[test]
fn register_outputs() -> Result<()> {
    let root = tempfile::tempdir().unwrap();
    let render = root.path().join("render.dat");
    let thumbnail = root.path().join("thumbnail.dat");
    std::fs::write(&render, b"fixture render").unwrap();
    std::fs::write(&thumbnail, b"fixture thumbnail").unwrap();
    let mut production = SqliteProduction::create(root.path().join("outputs.pproj"), None)?;
    // [register-outputs]
    let mut recipe = OutputRecipe::new(
        ActivityKind::new("example:generate")?,
        std::iter::empty::<postproject_core::RepresentationId>(),
    )?;
    recipe.tool = Some(ToolIdentity::new(
        "Fixture generator",
        Some("1.0".to_owned()),
        None,
    )?);
    recipe.parameters.push((
        MetadataProperty::new(
            VocabularyId::new("example:recipe")?,
            PropertyId::new("width")?,
        ),
        MetadataValue::i64(1920),
    ));
    let prepared = prepare_outputs(
        &[
            OutputSource::new(
                None,
                RepresentationKind::Original,
                MediaSource::File(render),
            ),
            OutputSource::new(
                None,
                RepresentationKind::Original,
                MediaSource::File(thumbnail),
            ),
        ],
        recipe,
    )?;
    let identities = prepared.identities();
    let activity = prepared.activity().id();
    // Media I/O finishes before acquiring the edit's write lock.
    let receipt = {
        let mut edit = prepared.stage(production.begin_transaction()?)?;
        edit.commit()?
    };
    // [/register-outputs]
    assert!(receipt.revision().is_some());
    assert_eq!(identities.len(), 2);
    assert_eq!(production.activities()?.len(), 1);
    assert_eq!(production.metadata(ObjectRef::Activity(activity))?.len(), 1);
    Ok(())
}
