//! Knowledge-only artifact staleness and divergence evaluation.

use std::fs;

use postproject_core::{
    Activity, ActivityId, ActivityInput, ActivityKind, ActivityOutput, ArtifactDependencyIssue,
    ArtifactEvaluationLimits, ArtifactKnowledgeReason, ArtifactKnowledgeState, Dependency,
    DependencyKind, DependencyTarget, RepresentationFingerprint, ResourceFingerprint,
};
use postproject_media::{fingerprint_representation, prepare_original_media};
use postproject_storage_sqlite::SqliteProduction;
use rusqlite::Connection;

#[test]
fn changed_dependency_fingerprint_makes_render_stale_not_source() {
    let directory = tempfile::tempdir().expect("create fixture directory");
    let production_path = directory.path().join("changed-dependency.pproj");
    let paths =
        ["shot.usda", "character.usda", "render.mov"].map(|name| directory.path().join(name));
    for (index, path) in paths.iter().enumerate() {
        fs::write(path, format!("media-{index}")).expect("write media fixture");
    }
    let imports =
        paths.map(|path| prepare_original_media(path, None, None).expect("prepare media fixture"));
    let shot_id = imports[0].representation().id();
    let character_id = imports[1].representation().id();
    let render_id = imports[2].representation().id();
    let dependency = Dependency::new(
        None,
        DependencyKind::new("org.openusd:reference").expect("kind"),
        DependencyTarget::Asset(imports[1].asset().id()),
        Some(character_id),
        true,
        "../assets/Character.usda",
    )
    .expect("dependency");
    let mut production = SqliteProduction::create(&production_path, None).expect("create");
    let base = production.read_session().unwrap().decision_base();
    let mut transaction = production.begin_edit(base).expect("begin setup");
    for import in &imports {
        transaction.import_original(import).expect("stage import");
    }
    transaction
        .record_dependency_set(shot_id, &[dependency])
        .expect("record dependency");
    transaction.commit().expect("commit setup");
    drop(transaction);
    create_activity(
        &mut production,
        shot_id,
        render_id,
        "org.postproject:render",
    );
    assert_eq!(
        evaluate(&production, render_id).state(),
        ArtifactKnowledgeState::Current
    );

    let original = &imports[1].representation().fingerprints()[0];
    let changed = RepresentationFingerprint::new(
        original.algorithm(),
        original.version(),
        vec![0x42; original.value().len()],
    )
    .expect("changed fingerprint");
    let base = production.read_session().unwrap().decision_base();
    let mut transaction = production.begin_edit(base).expect("begin observation");
    transaction
        .record_representation_fingerprint(character_id, &changed)
        .expect("record character fingerprint");
    transaction.commit().expect("commit observation");
    drop(transaction);
    let render = evaluate(&production, render_id);
    assert_eq!(render.state(), ArtifactKnowledgeState::Stale);
    assert!(render.reasons().iter().any(|reason| matches!(
        reason,
        ArtifactKnowledgeReason::DependencyFingerprintChanged {
            representation_id,
            path,
            ..
        } if *representation_id == character_id
            && path.len() == 1
            && path[0].source_representation_id() == shot_id
            && path[0].kind().as_str() == "org.openusd:reference"
    )));
    assert_ne!(
        evaluate(&production, shot_id).state(),
        ArtifactKnowledgeState::Stale
    );
}

#[test]
fn changed_authored_dependency_path_makes_artifact_stale() {
    let directory = tempfile::tempdir().expect("create fixture directory");
    let production_path = directory.path().join("changed-dependency-path.pproj");
    let paths =
        ["shot.usda", "character.usda", "render.mov"].map(|name| directory.path().join(name));
    for (index, path) in paths.iter().enumerate() {
        fs::write(path, format!("media-{index}")).expect("write media fixture");
    }
    let imports =
        paths.map(|path| prepare_original_media(path, None, None).expect("prepare media fixture"));
    let shot_id = imports[0].representation().id();
    let render_id = imports[2].representation().id();
    let dependency = Dependency::new(
        None,
        DependencyKind::new("org.openusd:reference").expect("kind"),
        DependencyTarget::Representation(imports[1].representation().id()),
        None,
        true,
        "../assets/Character.usda",
    )
    .expect("dependency");
    let mut production = SqliteProduction::create(&production_path, None).expect("create");
    let base = production.read_session().unwrap().decision_base();
    let mut transaction = production.begin_edit(base).expect("begin setup");
    for import in &imports {
        transaction.import_original(import).expect("stage import");
    }
    transaction
        .record_dependency_set(shot_id, &[dependency])
        .expect("record dependency");
    transaction.commit().expect("commit setup");
    drop(transaction);
    create_activity(
        &mut production,
        shot_id,
        render_id,
        "org.postproject:render",
    );
    assert_eq!(
        evaluate(&production, render_id).state(),
        ArtifactKnowledgeState::Current
    );

    let base = production.read_session().unwrap().decision_base();
    let mut transaction = production.begin_edit(base).expect("begin replacement");
    transaction
        .record_dependency_set(shot_id, &[])
        .expect("remove dependency");
    transaction.commit().expect("commit replacement");
    drop(transaction);
    let evaluation = evaluate(&production, render_id);
    assert_eq!(evaluation.state(), ArtifactKnowledgeState::Stale);
    assert!(evaluation.reasons().iter().any(|reason| matches!(
        reason,
        ArtifactKnowledgeReason::DependencyPathChanged { path, .. }
            if path.len() == 1
                && path[0].authored_reference() == "../assets/Character.usda"
    )));
}

#[test]
fn unresolved_dependency_snapshot_is_indeterminate_with_its_authored_path() {
    let directory = tempfile::tempdir().expect("create fixture directory");
    let production_path = directory.path().join("unresolved-dependency.pproj");
    let paths =
        ["shot.usda", "character.usda", "render.mov"].map(|name| directory.path().join(name));
    for (index, path) in paths.iter().enumerate() {
        fs::write(path, format!("media-{index}")).expect("write media fixture");
    }
    let imports =
        paths.map(|path| prepare_original_media(path, None, None).expect("prepare media fixture"));
    let shot_id = imports[0].representation().id();
    let render_id = imports[2].representation().id();
    let dependency = Dependency::new(
        None,
        DependencyKind::new("org.openusd:reference").expect("kind"),
        DependencyTarget::Asset(imports[1].asset().id()),
        None,
        true,
        "../assets/Character.usda",
    )
    .expect("unresolved dependency");
    let mut production = SqliteProduction::create(&production_path, None).expect("create");
    let base = production.read_session().unwrap().decision_base();
    let mut transaction = production.begin_edit(base).expect("begin setup");
    for import in &imports {
        transaction.import_original(import).expect("stage import");
    }
    transaction
        .record_dependency_set(shot_id, &[dependency])
        .expect("record dependency");
    transaction.commit().expect("commit setup");
    drop(transaction);
    create_activity(
        &mut production,
        shot_id,
        render_id,
        "org.postproject:render",
    );

    let evaluation = evaluate(&production, render_id);
    assert_eq!(evaluation.state(), ArtifactKnowledgeState::Indeterminate);
    assert!(evaluation.reasons().iter().any(|reason| matches!(
        reason,
        ArtifactKnowledgeReason::DependencyKnowledgeIncomplete {
            issue: ArtifactDependencyIssue::Unresolved,
            path,
            ..
        } if path.len() == 1
            && path[0].kind().as_str() == "org.openusd:reference"
            && path[0].authored_reference() == "../assets/Character.usda"
    )));
}

#[test]
fn legacy_activity_inputs_report_absent_dependency_evidence() {
    let directory = tempfile::tempdir().expect("create fixture directory");
    let production_path = directory.path().join("legacy-dependency-snapshot.pproj");
    let paths = ["source.mov", "output.mov"].map(|name| directory.path().join(name));
    for (index, path) in paths.iter().enumerate() {
        fs::write(path, format!("media-{index}")).expect("write media fixture");
    }
    let imports =
        paths.map(|path| prepare_original_media(path, None, None).expect("prepare media fixture"));
    let source_id = imports[0].representation().id();
    let output_id = imports[1].representation().id();
    let mut production = SqliteProduction::create(&production_path, None).expect("create");
    let mut transaction = production.begin_transaction().expect("begin setup");
    for import in &imports {
        transaction.import_original(import).expect("stage import");
    }
    transaction.commit().expect("commit imports");
    drop(transaction);
    create_activity(
        &mut production,
        source_id,
        output_id,
        "org.postproject:derive",
    );
    assert_eq!(
        evaluate(&production, output_id).state(),
        ArtifactKnowledgeState::Current
    );
    drop(production);

    let connection = Connection::open(&production_path).expect("open raw fixture");
    connection
        .execute("DELETE FROM activity_input_dependency_snapshots", [])
        .expect("simulate pre-dependency activity");
    drop(connection);
    let production = SqliteProduction::open(&production_path).expect("reopen fixture");
    let evaluation = evaluate(&production, output_id);
    assert_eq!(evaluation.state(), ArtifactKnowledgeState::Indeterminate);
    assert!(evaluation.reasons().iter().any(|reason| matches!(
        reason,
        ArtifactKnowledgeReason::DependencySnapshotAbsent {
            representation_id,
            ..
        } if *representation_id == source_id
    )));
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one test follows the complete current-to-stale-to-diverged lifecycle"
)]
fn changed_source_propagates_staleness_and_changed_output_diverges() {
    let directory = tempfile::tempdir().expect("create fixture directory");
    let production_path = directory.path().join("artifacts.pproj");
    let paths = ["source.mov", "proxy.mov", "render.mov"].map(|name| directory.path().join(name));
    for (index, path) in paths.iter().enumerate() {
        fs::write(path, format!("media-{index}")).expect("write media fixture");
    }
    let imports =
        paths.map(|path| prepare_original_media(path, None, None).expect("prepare media fixture"));
    let source_id = imports[0].representation().id();
    let proxy_id = imports[1].representation().id();
    let render_id = imports[2].representation().id();

    let mut production = SqliteProduction::create(&production_path, None).expect("create");
    for import in &imports {
        let mut transaction = production.begin_transaction().expect("begin import");
        transaction.import_original(import).expect("stage import");
        transaction.commit().expect("commit import");
    }
    create_activity(
        &mut production,
        source_id,
        proxy_id,
        "org.postproject:generate-proxy",
    );
    create_activity(
        &mut production,
        proxy_id,
        render_id,
        "org.postproject:render",
    );

    assert_eq!(
        evaluate(&production, proxy_id).state(),
        ArtifactKnowledgeState::Current
    );
    assert_eq!(
        evaluate(&production, render_id).state(),
        ArtifactKnowledgeState::Current
    );

    let source_resource = imports[0].resources()[0].id();
    let original = &imports[0].resources()[0].fingerprints()[0];
    let changed = ResourceFingerprint::new(
        original.algorithm(),
        original.version(),
        vec![0x55; original.value().len()],
    )
    .expect("changed resource fingerprint");
    let base = production.read_session().unwrap().decision_base();
    let mut transaction = production.begin_edit(base).expect("begin observation");
    transaction
        .record_resource_fingerprint(source_resource, &changed)
        .expect("record resource fingerprint");
    transaction.commit().expect("commit resource fingerprint");
    drop(transaction);

    let proxy = evaluate(&production, proxy_id);
    assert_eq!(proxy.state(), ArtifactKnowledgeState::Stale);
    assert!(proxy.reasons().iter().any(|reason| matches!(
        reason,
        ArtifactKnowledgeReason::FingerprintRecomputationPending {
            representation_id,
            ..
        } if *representation_id == source_id
    )));
    let render = evaluate(&production, render_id);
    assert_eq!(render.state(), ArtifactKnowledgeState::Stale);
    assert!(render.reasons().iter().any(|reason| matches!(
        reason,
        ArtifactKnowledgeReason::UpstreamNotCurrent {
            representation_id,
            state: ArtifactKnowledgeState::Stale,
        } if *representation_id == proxy_id
    )));

    let source = &production
        .representations(imports[0].asset().id())
        .expect("load source representation")[0];
    let resources = production
        .resources(source_id)
        .expect("load source resources");
    let aggregate = fingerprint_representation(source.content_structure(), &resources)
        .expect("recompute representation fingerprint");
    let base = production.read_session().unwrap().decision_base();
    let mut transaction = production.begin_edit(base).expect("begin recompute");
    transaction
        .record_representation_fingerprint(source_id, &aggregate)
        .expect("record representation fingerprint");
    transaction.commit().expect("commit recomputation");
    drop(transaction);
    assert!(
        evaluate(&production, proxy_id)
            .reasons()
            .iter()
            .any(|reason| matches!(reason, ArtifactKnowledgeReason::FingerprintChanged { .. }))
    );

    let proxy_fingerprint = &imports[1].representation().fingerprints()[0];
    let diverged = RepresentationFingerprint::new(
        proxy_fingerprint.algorithm(),
        proxy_fingerprint.version(),
        vec![0x77; proxy_fingerprint.value().len()],
    )
    .expect("diverged fingerprint");
    let base = production.read_session().unwrap().decision_base();
    let mut transaction = production.begin_edit(base).expect("begin divergence");
    transaction
        .record_representation_fingerprint(proxy_id, &diverged)
        .expect("record divergence");
    transaction.commit().expect("commit divergence");
    drop(transaction);
    assert_eq!(
        evaluate(&production, proxy_id).state(),
        ArtifactKnowledgeState::Diverged
    );
    assert_eq!(
        evaluate(&production, render_id).state(),
        ArtifactKnowledgeState::Stale
    );
}

#[test]
fn fifty_deep_chain_is_current_and_smaller_bounds_truncate_explicitly() {
    let directory = tempfile::tempdir().expect("create fixture directory");
    let production_path = directory.path().join("deep-artifacts.pproj");
    let mut imports = Vec::new();
    for index in 0..=50 {
        let path = directory.path().join(format!("artifact-{index}.mov"));
        fs::write(&path, format!("media-{index}")).expect("write media fixture");
        imports.push(prepare_original_media(&path, None, None).expect("prepare media fixture"));
    }

    let mut production = SqliteProduction::create(&production_path, None).expect("create");
    let mut transaction = production.begin_transaction().expect("begin imports");
    for import in &imports {
        transaction.import_original(import).expect("stage import");
    }
    transaction.commit().expect("commit imports");
    drop(transaction);

    let mut transaction = production.begin_transaction().expect("begin activities");
    for pair in imports.windows(2) {
        let activity = Activity::new(
            ActivityId::new(),
            ActivityKind::new("org.postproject:derive").expect("valid activity kind"),
            vec![ActivityInput::new(pair[0].representation().id(), None)],
            vec![ActivityOutput::new(pair[1].representation().id(), None)],
        )
        .expect("valid activity");
        transaction
            .create_activity(&activity)
            .expect("stage activity");
    }
    transaction.commit().expect("commit activities");
    drop(transaction);

    let target = imports[50].representation().id();
    let complete = evaluate(&production, target);
    assert_eq!(complete.state(), ArtifactKnowledgeState::Current);
    assert_eq!(complete.visited_representations(), 50);
    assert!(!complete.is_truncated());

    let bounded = production
        .evaluate_artifact(
            target,
            ArtifactEvaluationLimits::new(10, 100).expect("valid limits"),
        )
        .expect("evaluate bounded chain");
    assert_eq!(bounded.state(), ArtifactKnowledgeState::Indeterminate);
    assert!(bounded.is_truncated());
    assert!(
        bounded
            .reasons()
            .iter()
            .any(|reason| matches!(reason, ArtifactKnowledgeReason::TraversalTruncated { .. }))
    );
}

fn create_activity(
    production: &mut SqliteProduction,
    input: postproject_core::RepresentationId,
    output: postproject_core::RepresentationId,
    kind: &str,
) {
    let activity = Activity::new(
        ActivityId::new(),
        ActivityKind::new(kind).expect("valid activity kind"),
        vec![ActivityInput::new(input, None)],
        vec![ActivityOutput::new(output, None)],
    )
    .expect("valid activity");
    let mut transaction = production.begin_transaction().expect("begin activity");
    transaction
        .create_activity(&activity)
        .expect("stage activity");
    transaction.commit().expect("commit activity");
}

fn evaluate(
    production: &SqliteProduction,
    representation_id: postproject_core::RepresentationId,
) -> postproject_core::ArtifactEvaluation {
    production
        .evaluate_artifact(representation_id, ArtifactEvaluationLimits::default())
        .expect("evaluate artifact")
}
