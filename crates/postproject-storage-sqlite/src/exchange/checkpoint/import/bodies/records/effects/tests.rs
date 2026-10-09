use postproject_core::{MediaRoot, MediaRootId};
use postproject_protocol::{
    ChunkSummary, Extensions, MediaChange, RecordFeature, RecordManifest, encode_event,
};

use super::RetainedEffects;

use crate::SqliteProduction;
use crate::exchange::checkpoint::import::{guard_state, root_state};

#[test]
fn every_retained_observation_needs_an_authored_effect_explanation() {
    for count in [1, 2] {
        let directory = tempfile::tempdir().unwrap();
        let mut source =
            SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
        let root = MediaRoot::new(MediaRootId::new(), "rushes", None, None, 0, true).unwrap();
        let base = source.read_session().unwrap().decision_base();
        let mut edit = source.begin_edit(base).unwrap();
        edit.add_media_root(root.clone()).unwrap();
        edit.set_media_root_enabled(root.id(), false).unwrap();
        edit.commit().unwrap();
        drop(edit);
        let revision = source.latest_revision().unwrap().unwrap();
        let events = source.events_for_revision(revision.id()).unwrap();
        assert_eq!(events.len(), 2);
        let manifest = RecordManifest::new(
            source.exchange_floor().unwrap(),
            revision,
            ChunkSummary::new(1, 10, source.exchange_head().unwrap().digest()).unwrap(),
            count,
            2,
            Extensions::default(),
        )
        .unwrap()
        .with_required_features([RecordFeature::Media, RecordFeature::RecordChunks])
        .unwrap();
        let connection = &source.connection;
        root_state::create(connection).unwrap();
        guard_state::create(connection).unwrap();
        let mut effects = RetainedEffects::new(&manifest, 0);
        effects
            .document(
                connection,
                &MediaChange::RootAdded(root.clone()).document().unwrap(),
            )
            .unwrap();
        if count == 2 {
            effects
                .document(
                    connection,
                    &MediaChange::RootEnabled {
                        root_id: root.id(),
                        enabled: false,
                    }
                    .document()
                    .unwrap(),
                )
                .unwrap();
        }
        for event in &events {
            effects
                .document(connection, &encode_event(event).unwrap())
                .unwrap();
        }
        let result = effects.finish();
        if count == 2 {
            result.unwrap();
        } else {
            assert!(
                matches!(result, Err(crate::ExchangeError::Protocol(error)) if error.kind() == postproject_protocol::FailureKind::Integrity)
            );
        }
    }
}
