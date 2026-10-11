use super::Scenario;
use crate::SqliteProduction;

pub(super) fn assert_public_secrets_absent(scenario: &Scenario<'_>, tokens: &[String]) {
    let mut chunks = vec![];
    let manifest = scenario
        .source
        .export_checkpoint(|chunk| {
            chunks.push(chunk);
            Ok(())
        })
        .unwrap();
    let mut public = manifest.document().unwrap().canonical_bytes().unwrap();
    let mut bodies = vec![];
    for chunk in &chunks {
        public.extend(chunk.document().unwrap().canonical_bytes().unwrap());
        bodies.extend_from_slice(chunk.payload());
    }
    public.extend(bodies);
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("mirror.pproj");
    let mirror = SqliteProduction::import_checkpoint(
        &destination,
        &manifest,
        chunks.into_iter().map(Ok),
        crate::CheckpointLimits::default(),
    )
    .unwrap();
    assert_eq!(
        mirror.exchange_head().unwrap(),
        scenario.source.exchange_head().unwrap()
    );
    drop(mirror);
    let database = std::fs::read(destination).unwrap();
    for token in tokens {
        let secret = token.rsplit(':').next().unwrap();
        let identity: postproject_core::JobClaimId = secret.parse().unwrap();
        for bytes in [secret.as_bytes(), identity.as_bytes().as_slice()] {
            assert!(!public.windows(bytes.len()).any(|window| window == bytes));
            assert!(!database.windows(bytes.len()).any(|window| window == bytes));
        }
    }
}
