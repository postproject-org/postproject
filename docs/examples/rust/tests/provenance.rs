//! Runs the Rust listings of the provenance and artifact-knowledge guide.
//!
//! Each `// [name]` ... `// [/name]` region is included verbatim by the
//! documentation build, so keep regions self-contained and readable.

use std::fs;
use std::path::{Path, PathBuf};

use postproject_core::{
    Activity, ActivityId, ActivityInput, ActivityKind, ActivityOutput, ActivityRole, AgentIdentity,
    ArtifactEvaluationLimits, ArtifactKnowledgeReason, ArtifactKnowledgeState,
    ArtifactReproducibilityIssue, AssetId, Dependency, DependencyKind, DependencyQueryLimits,
    DependencySetStatus, DependencyTarget, ExternalIdentifier, IdentifierScheme,
    ProvenanceQueryLimits, QueryPageRequest, RepresentationId, RepresentationKind, Result,
    ToolIdentity,
};
use postproject_media::{
    fingerprint_file, fingerprint_representation, prepare_original_media, prepare_representation,
};
use postproject_storage_sqlite::SqliteProduction;

// [activity-snapshots]
fn record_proxy_generation(
    production: &mut SqliteProduction,
    original_id: RepresentationId,
    proxy_id: RepresentationId,
) -> Result<ActivityId> {
    let activity = Activity::new(
        ActivityId::new(),
        ActivityKind::new("org.postproject:generate-proxy")?,
        vec![ActivityInput::new(
            original_id,
            Some(ActivityRole::new("org.postproject:primary")?),
        )],
        vec![ActivityOutput::new(proxy_id, None)],
    )?
    .with_tool(ToolIdentity::new(
        "Example Encoder",
        Some("3.2".to_owned()),
        None,
    )?)
    .with_agent(AgentIdentity::new(
        Some("Render node 4".to_owned()),
        Some(ExternalIdentifier::new(
            IdentifierScheme::new("com.example.render-node")?,
            "node-4",
            None,
        )?),
    )?);
    {
        let mut transaction = production.begin_transaction()?;
        // Storage captures the current input and output fingerprints on each edge.
        transaction.create_activity(&activity)?;
        transaction.commit()?;
    }

    for recorded in production.activities()? {
        println!("activity {} ({})", recorded.id(), recorded.kind().as_str());
        if let Some(tool) = recorded.tool() {
            println!("  tool: {} {:?}", tool.name(), tool.version());
        }
        if let Some(agent) = recorded.agent() {
            println!("  agent: {:?}", agent.name());
        }
        for input in recorded.inputs() {
            println!(
                "  input {} as {:?}",
                input.representation_id(),
                input.role()
            );
            for fingerprint in input
                .snapshot()
                .map_or(&[][..], |snapshot| snapshot.fingerprints())
            {
                println!("    {} v{}", fingerprint.algorithm(), fingerprint.version());
            }
        }
        for output in recorded.outputs() {
            let snapshot = output.snapshot().expect("storage records output snapshots");
            println!(
                "  output {} at revision {}",
                output.representation_id(),
                snapshot.revision_sequence()
            );
        }
    }

    let consuming = production.activities_consuming(original_id)?;
    println!("activities consuming the original: {}", consuming.len());

    let descendants = production.descendants(original_id)?;
    let page = production.descendants_page(
        original_id,
        ProvenanceQueryLimits::new(8, 1_000)?,
        &QueryPageRequest::new(100, None)?,
    )?;
    for item in page.items() {
        println!(
            "descendant {} at depth {}",
            item.representation_id(),
            item.depth()
        );
    }
    assert_eq!(descendants.len(), page.items().len());
    Ok(activity.id())
}
// [/activity-snapshots]

// [stale-after-change]
fn record_changed_file(
    production: &mut SqliteProduction,
    asset_id: AssetId,
    representation_id: RepresentationId,
    path: &Path,
) -> Result<()> {
    // Record the new resource observation first.
    let resource_id = production.resources(representation_id)?[0].id();
    let observed = fingerprint_file(path)?;
    {
        let mut transaction = production.begin_transaction()?;
        transaction.record_resource_fingerprint(resource_id, observed.fingerprint())?;
        transaction.commit()?;
    }

    // Then recompute the representation fingerprint from current resources.
    let representation = production
        .representations(asset_id)?
        .into_iter()
        .find(|representation| representation.id() == representation_id)
        .expect("representation belongs to the asset");
    let aggregate = fingerprint_representation(
        representation.content_structure(),
        &production.resources(representation_id)?,
    )?;
    let mut transaction = production.begin_transaction()?;
    transaction.record_representation_fingerprint(representation_id, &aggregate)?;
    transaction.commit()
}

fn evaluate_proxy(
    production: &SqliteProduction,
    proxy_id: RepresentationId,
) -> Result<ArtifactKnowledgeState> {
    // Evaluation reads recorded knowledge only; it never touches media files.
    let evaluation =
        production.evaluate_artifact(proxy_id, ArtifactEvaluationLimits::new(64, 1_000)?)?;
    for reason in evaluation.reasons() {
        match reason {
            ArtifactKnowledgeReason::FingerprintChanged {
                representation_id,
                algorithm,
                ..
            } => println!("{representation_id} changed ({algorithm})"),
            other => println!("reason: {other:?}"),
        }
    }

    let reproducibility = production.artifact_reproducibility(proxy_id)?;
    for issue in reproducibility.issues() {
        match issue {
            ArtifactReproducibilityIssue::ParametersMissing { activity_id } => {
                println!("activity {activity_id} recorded no parameters");
            }
            other => println!("reproducibility issue: {other:?}"),
        }
    }
    Ok(evaluation.state())
}
// [/stale-after-change]

// [dependency-set]
fn record_comp_dependencies(
    production: &mut SqliteProduction,
    comp_id: RepresentationId,
    plate_asset_id: AssetId,
    plate_original_id: RepresentationId,
    proxy_id: RepresentationId,
) -> Result<()> {
    let dependencies = [
        // A floating asset reference, recorded with what it resolved to.
        Dependency::new(
            None,
            DependencyKind::new("org.example:plate")?,
            DependencyTarget::Asset(plate_asset_id),
            Some(plate_original_id),
            true,
            "rushes/A001.mov",
        )?,
        // A reference pinned to one representation.
        Dependency::new(
            None,
            DependencyKind::new("org.example:preview")?,
            DependencyTarget::Representation(proxy_id),
            None,
            false,
            "proxies/A001_proxy.mov",
        )?,
    ];
    {
        let mut transaction = production.begin_transaction()?;
        // The set replaces the previous observation as a whole.
        transaction.record_dependency_set(comp_id, &dependencies)?;
        transaction.commit()?;
    }

    let set = production
        .dependency_set(comp_id)?
        .expect("a dependency set was recorded");
    println!(
        "{} dependencies recorded at revision {}: {:?}",
        set.dependencies().len(),
        set.recorded_at_revision(),
        set.status()
    );
    for dependency in set.dependencies() {
        println!(
            "{} -> {:?} ({})",
            dependency.kind().as_str(),
            dependency.target(),
            dependency.authored_reference()
        );
    }

    let limits = DependencyQueryLimits::new(4, 1_000)?;
    let mut cursor = None;
    loop {
        let page = production.dependencies(comp_id, limits, &QueryPageRequest::new(1, cursor)?)?;
        for item in page.items() {
            println!("depends on {:?} at depth {}", item.target(), item.depth());
        }
        cursor = page.next_cursor().cloned();
        if cursor.is_none() {
            return Ok(());
        }
    }
}

fn dependency_status(
    production: &SqliteProduction,
    comp_id: RepresentationId,
) -> Result<Option<DependencySetStatus>> {
    // After a new fingerprint observation of the comp, the recorded set is
    // reported as needing extraction until it is recorded again.
    Ok(production.dependency_set(comp_id)?.map(|set| set.status()))
}
// [/dependency-set]

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../examples/fixtures/sample-media.dat")
}

#[test]
fn provenance_examples_run_in_order() -> Result<()> {
    let work = tempfile::tempdir().expect("temporary directory");
    let rushes = work.path().join("rushes");
    let proxies = work.path().join("proxies");
    let comps = work.path().join("comps");
    for directory in [&rushes, &proxies, &comps] {
        fs::create_dir_all(directory).expect("work directory");
    }
    let original_path = rushes.join("A001.mov");
    let proxy_path = proxies.join("A001_proxy.mov");
    let comp_path = comps.join("shot010.nk");
    fs::copy(fixture(), &original_path).expect("media fixture");
    fs::write(&proxy_path, "proxy of A001").expect("proxy file");
    fs::write(&comp_path, "comp script v1").expect("comp file");

    let mut production = SqliteProduction::create(work.path().join("provenance.pproj"), None)?;
    let original = prepare_original_media(&original_path, Some("A001".to_owned()), None)?;
    let asset_id = original.asset().id();
    let original_id = original.representation().id();
    let proxy = prepare_representation(asset_id, RepresentationKind::Proxy, &proxy_path)?;
    let proxy_id = proxy.representation().id();
    let comp = prepare_original_media(&comp_path, Some("shot010 comp".to_owned()), None)?;
    let comp_asset_id = comp.asset().id();
    let comp_id = comp.representation().id();
    {
        let mut transaction = production.begin_transaction()?;
        transaction.import_original(&original)?;
        transaction.add_representation(&proxy)?;
        transaction.import_original(&comp)?;
        transaction.commit()?;
    }

    let activity_id = record_proxy_generation(&mut production, original_id, proxy_id)?;
    let activities = production.activities()?;
    assert_eq!(activities.len(), 1);
    let activity = &activities[0];
    assert_eq!(activity.id(), activity_id);
    assert_eq!(
        activity.tool().map(ToolIdentity::name),
        Some("Example Encoder")
    );
    assert_eq!(
        activity.agent().and_then(AgentIdentity::name),
        Some("Render node 4")
    );
    let input_snapshot = activity.inputs()[0].snapshot().expect("input snapshot");
    assert!(!input_snapshot.fingerprints().is_empty());
    assert!(activity.outputs()[0].snapshot().is_some());
    assert_eq!(
        production.activities_consuming(original_id)?[0].id(),
        activity_id
    );
    assert_eq!(production.descendants(original_id)?, vec![proxy_id]);
    assert_eq!(
        evaluate_proxy(&production, proxy_id)?,
        ArtifactKnowledgeState::Current
    );

    // The camera original is re-exported over the same path.
    fs::write(&original_path, "re-graded camera original").expect("overwrite original");
    record_changed_file(&mut production, asset_id, original_id, &original_path)?;
    assert_eq!(
        evaluate_proxy(&production, proxy_id)?,
        ArtifactKnowledgeState::Stale
    );
    let evaluation =
        production.evaluate_artifact(proxy_id, ArtifactEvaluationLimits::new(64, 1_000)?)?;
    assert!(evaluation.reasons().iter().any(|reason| matches!(
        reason,
        ArtifactKnowledgeReason::FingerprintChanged { representation_id, .. }
            if *representation_id == original_id
    )));
    let reproducibility = production.artifact_reproducibility(proxy_id)?;
    assert!(!reproducibility.is_reproducible());
    assert!(
        reproducibility
            .issues()
            .contains(&ArtifactReproducibilityIssue::ParametersMissing { activity_id })
    );

    record_comp_dependencies(&mut production, comp_id, asset_id, original_id, proxy_id)?;
    let set = production.dependency_set(comp_id)?.expect("dependency set");
    assert_eq!(set.dependencies().len(), 2);
    assert_eq!(set.status(), DependencySetStatus::Current);
    let all = production.dependencies(
        comp_id,
        DependencyQueryLimits::new(4, 1_000)?,
        &QueryPageRequest::new(100, None)?,
    )?;
    assert_eq!(all.items().len(), 2);

    fs::write(&comp_path, "comp script v2").expect("edit comp");
    record_changed_file(&mut production, comp_asset_id, comp_id, &comp_path)?;
    assert_eq!(
        dependency_status(&production, comp_id)?,
        Some(DependencySetStatus::NeedsExtraction)
    );
    Ok(())
}
