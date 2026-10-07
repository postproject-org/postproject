//! Evaluation of dependency evidence captured on activity inputs.

use std::collections::{BTreeMap, BTreeSet};

use postproject_core::{
    Activity, ActivityInput, ArtifactDependencyIssue, ArtifactDependencyPathSegment,
    ArtifactKnowledgeReason, ArtifactKnowledgeState, AssetId, Dependency, DependencyKind,
    DependencySetStatus, DependencyTarget, Error, ErrorKind, Representation, RepresentationId,
    ResourceId, Result,
};
use rusqlite::params;

use crate::{SqliteProduction, id_bytes, read_budget::ReadBudget, sqlite_error};

struct CapturedPath {
    status: i64,
    subject_representation_id: RepresentationId,
    segments: Vec<ArtifactDependencyPathSegment>,
    fingerprints: Fingerprints,
}

type Fingerprints = BTreeMap<(String, u16), Vec<u8>>;
type FingerprintDifference = (String, u16, Option<Vec<u8>>, Option<Vec<u8>>);

pub(crate) struct DependencyEvaluation {
    pub(crate) state: ArtifactKnowledgeState,
    pub(crate) reasons: Vec<ArtifactKnowledgeReason>,
}

pub(crate) fn evaluate_input_dependencies(
    production: &SqliteProduction,
    activity: &Activity,
    input: &ActivityInput,
) -> Result<DependencyEvaluation> {
    let input_id = production
        .connection
        .query_row(
            "SELECT id FROM activity_inputs
             WHERE activity_id = ?1 AND representation_id = ?2 AND role IS ?3",
            params![
                activity.id().as_bytes().as_slice(),
                input.representation_id().as_bytes().as_slice(),
                input.role().map(postproject_core::ActivityRole::as_str),
            ],
            |row| row.get::<_, i64>(0),
        )
        .map_err(sqlite_error("locate activity input dependency snapshot"))?;
    let snapshot_exists = production
        .connection
        .query_row(
            "SELECT EXISTS (
                SELECT 1 FROM activity_input_dependency_snapshots
                WHERE activity_input_id = ?1
             )",
            [input_id],
            |row| row.get::<_, bool>(0),
        )
        .map_err(sqlite_error(
            "read activity input dependency snapshot marker",
        ))?;
    if !snapshot_exists {
        return Ok(DependencyEvaluation {
            state: ArtifactKnowledgeState::Indeterminate,
            reasons: vec![ArtifactKnowledgeReason::DependencySnapshotAbsent {
                activity_id: activity.id(),
                representation_id: input.representation_id(),
            }],
        });
    }
    let mut evaluation = DependencyEvaluation {
        state: ArtifactKnowledgeState::Current,
        reasons: Vec::new(),
    };
    for path in load_paths(production, input_id)? {
        evaluate_path(production, activity, input, path, &mut evaluation)?;
    }
    Ok(evaluation)
}

fn evaluate_path(
    production: &SqliteProduction,
    activity: &Activity,
    input: &ActivityInput,
    path: CapturedPath,
    evaluation: &mut DependencyEvaluation,
) -> Result<()> {
    if let Some(issue) = captured_issue(path.status)? {
        promote_state(&mut evaluation.state, ArtifactKnowledgeState::Indeterminate);
        evaluation
            .reasons
            .push(ArtifactKnowledgeReason::DependencyKnowledgeIncomplete {
                activity_id: activity.id(),
                input_representation_id: input.representation_id(),
                subject_representation_id: path.subject_representation_id,
                path: path.segments,
                issue,
            });
        return Ok(());
    }
    for (index, segment) in path.segments.iter().enumerate() {
        let Some(set) =
            crate::load_dependency_set(&production.connection, segment.source_representation_id())?
        else {
            add_path_changed(activity, input, &path, evaluation);
            return Ok(());
        };
        if set.status() == DependencySetStatus::NeedsExtraction {
            promote_state(&mut evaluation.state, ArtifactKnowledgeState::Indeterminate);
            evaluation
                .reasons
                .push(ArtifactKnowledgeReason::DependencyKnowledgeIncomplete {
                    activity_id: activity.id(),
                    input_representation_id: input.representation_id(),
                    subject_representation_id: segment.source_representation_id(),
                    path: path.segments[..index].to_vec(),
                    issue: ArtifactDependencyIssue::NeedsExtraction,
                });
            return Ok(());
        }
        let dependency_position =
            usize::try_from(segment.dependency_position()).map_err(|error| {
                Error::new(
                    ErrorKind::Storage,
                    format!("dependency position cannot be addressed: {error}"),
                )
            })?;
        if !set
            .dependencies()
            .get(dependency_position)
            .is_some_and(|dependency| segment_matches(segment, dependency))
        {
            add_path_changed(activity, input, &path, evaluation);
            return Ok(());
        }
    }
    if representation_fingerprint_dirty(production, path.subject_representation_id)? {
        promote_state(&mut evaluation.state, ArtifactKnowledgeState::Stale);
        evaluation.reasons.push(
            ArtifactKnowledgeReason::DependencyFingerprintRecomputationPending {
                activity_id: activity.id(),
                input_representation_id: input.representation_id(),
                representation_id: path.subject_representation_id,
                path: path.segments,
            },
        );
        return Ok(());
    }
    compare_fingerprints(production, activity, input, &path, evaluation)
}

fn segment_matches(segment: &ArtifactDependencyPathSegment, dependency: &Dependency) -> bool {
    dependency.is_required()
        && segment.source_resource_id() == dependency.source_resource_id()
        && segment.kind() == dependency.kind()
        && segment.target() == dependency.target()
        && segment.resolved_representation_id() == dependency.resolved_representation_id()
        && segment.authored_reference() == dependency.authored_reference()
}

fn add_path_changed(
    activity: &Activity,
    input: &ActivityInput,
    path: &CapturedPath,
    evaluation: &mut DependencyEvaluation,
) {
    promote_state(&mut evaluation.state, ArtifactKnowledgeState::Stale);
    evaluation
        .reasons
        .push(ArtifactKnowledgeReason::DependencyPathChanged {
            activity_id: activity.id(),
            input_representation_id: input.representation_id(),
            path: path.segments.clone(),
        });
}

fn compare_fingerprints(
    production: &SqliteProduction,
    activity: &Activity,
    input: &ActivityInput,
    path: &CapturedPath,
    evaluation: &mut DependencyEvaluation,
) -> Result<()> {
    let current = current_fingerprints(
        &production.load_representation_by_id(path.subject_representation_id)?,
    );
    let domains = path
        .fingerprints
        .keys()
        .chain(current.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    if domains.is_empty() {
        add_missing_fingerprint(activity, input, path, None, evaluation);
        return Ok(());
    }
    for (algorithm, version) in domains {
        match (
            path.fingerprints.get(&(algorithm.clone(), version)),
            current.get(&(algorithm.clone(), version)),
        ) {
            (Some(snapshot), Some(current)) if snapshot != current => {
                promote_state(&mut evaluation.state, ArtifactKnowledgeState::Stale);
                evaluation
                    .reasons
                    .push(ArtifactKnowledgeReason::DependencyFingerprintChanged {
                        activity_id: activity.id(),
                        input_representation_id: input.representation_id(),
                        representation_id: path.subject_representation_id,
                        path: path.segments.clone(),
                        algorithm,
                        version,
                        snapshot_value: snapshot.clone(),
                        current_value: current.clone(),
                    });
            }
            (Some(_), Some(_)) => {}
            (snapshot, current) => add_missing_fingerprint(
                activity,
                input,
                path,
                Some((algorithm, version, snapshot.cloned(), current.cloned())),
                evaluation,
            ),
        }
    }
    Ok(())
}

fn add_missing_fingerprint(
    activity: &Activity,
    input: &ActivityInput,
    path: &CapturedPath,
    difference: Option<FingerprintDifference>,
    evaluation: &mut DependencyEvaluation,
) {
    promote_state(&mut evaluation.state, ArtifactKnowledgeState::Indeterminate);
    let (algorithm, version, snapshot_value, current_value) = difference.map_or(
        (None, None, None, None),
        |(algorithm, version, snapshot, current)| {
            (Some(algorithm), Some(version), snapshot, current)
        },
    );
    evaluation.reasons.push(
        ArtifactKnowledgeReason::DependencyFingerprintEvidenceMissing {
            activity_id: activity.id(),
            input_representation_id: input.representation_id(),
            representation_id: path.subject_representation_id,
            path: path.segments.clone(),
            algorithm,
            version,
            snapshot_value,
            current_value,
        },
    );
}

fn load_paths(production: &SqliteProduction, input_id: i64) -> Result<Vec<CapturedPath>> {
    let mut budget = ReadBudget::default();
    let mut statement = production
        .connection
        .prepare(
            "SELECT id, position, status, subject_representation_id
             FROM activity_input_dependency_paths
             WHERE activity_input_id = ?1 ORDER BY position",
        )
        .map_err(sqlite_error("prepare dependency snapshot path query"))?;
    let rows = statement
        .query_map(
            [input_id],
            crate::read_budget::bounded_with(&mut budget, |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                ))
            }),
        )
        .map_err(sqlite_error("query dependency snapshot paths"))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(sqlite_error("read dependency snapshot path"))?;
    rows.into_iter()
        .enumerate()
        .map(
            |(expected_position, (path_id, position, status, subject))| {
                if usize::try_from(position).ok() != Some(expected_position) {
                    return Err(stored_invariant(
                        "dependency snapshot path positions are not contiguous",
                    ));
                }
                Ok(CapturedPath {
                    status,
                    subject_representation_id: RepresentationId::from_bytes(id_bytes(
                        subject,
                        "dependency snapshot subject representation",
                    )?),
                    segments: load_segments(production, path_id, &mut budget)?,
                    fingerprints: load_fingerprints(production, path_id, &mut budget)?,
                })
            },
        )
        .collect()
}

fn load_fingerprints(
    production: &SqliteProduction,
    path_id: i64,
    budget: &mut ReadBudget,
) -> Result<Fingerprints> {
    let mut statement = production
        .connection
        .prepare(
            "SELECT algorithm, algorithm_version, value
             FROM activity_input_dependency_fingerprint_snapshots
             WHERE path_id = ?1 ORDER BY algorithm, algorithm_version",
        )
        .map_err(sqlite_error(
            "prepare dependency fingerprint snapshot query",
        ))?;
    let rows = statement
        .query_map(
            [path_id],
            crate::read_budget::bounded_with(budget, |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                ))
            }),
        )
        .map_err(sqlite_error("query dependency fingerprint snapshots"))?;
    rows.map(|row| {
        let (algorithm, version, value) =
            row.map_err(sqlite_error("read dependency fingerprint snapshot"))?;
        let version = u16::try_from(version)
            .map_err(|_| stored_invariant("dependency fingerprint version is invalid"))?;
        Ok(((algorithm, version), value))
    })
    .collect()
}

fn load_segments(
    production: &SqliteProduction,
    path_id: i64,
    budget: &mut ReadBudget,
) -> Result<Vec<ArtifactDependencyPathSegment>> {
    let mut statement = production
        .connection
        .prepare(
            "SELECT position, source_representation_id, dependency_position,
                    source_resource_id, kind, target_kind, target_id,
                    resolved_representation_id, authored_reference
             FROM activity_input_dependency_path_edges
             WHERE path_id = ?1 ORDER BY position",
        )
        .map_err(sqlite_error("prepare dependency snapshot edge query"))?;
    let rows = statement
        .query_map(
            [path_id],
            crate::read_budget::bounded_with(budget, |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Option<Vec<u8>>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, Vec<u8>>(6)?,
                    row.get::<_, Option<Vec<u8>>>(7)?,
                    row.get::<_, String>(8)?,
                ))
            }),
        )
        .map_err(sqlite_error("query dependency snapshot edges"))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(sqlite_error("read dependency snapshot edge"))?;
    rows.into_iter()
        .enumerate()
        .map(|(expected_position, row)| decode_segment(expected_position, row))
        .collect()
}

#[allow(
    clippy::type_complexity,
    reason = "tuple mirrors one private storage row"
)]
fn decode_segment(
    expected_position: usize,
    row: (
        i64,
        Vec<u8>,
        i64,
        Option<Vec<u8>>,
        String,
        i64,
        Vec<u8>,
        Option<Vec<u8>>,
        String,
    ),
) -> Result<ArtifactDependencyPathSegment> {
    let (
        position,
        source,
        dependency_position,
        resource,
        kind,
        target_kind,
        target,
        resolved,
        authored,
    ) = row;
    if usize::try_from(position).ok() != Some(expected_position) {
        return Err(stored_invariant(
            "dependency snapshot edge positions are not contiguous",
        ));
    }
    let source = RepresentationId::from_bytes(id_bytes(source, "dependency path source")?);
    let dependency_position = u32::try_from(dependency_position)
        .map_err(|_| stored_invariant("dependency path source position is invalid"))?;
    let resource = resource
        .map(|value| id_bytes(value, "dependency path source resource").map(ResourceId::from_bytes))
        .transpose()?;
    let kind = DependencyKind::new(kind).map_err(stored_domain_error("dependency path kind"))?;
    let target = id_bytes(target, "dependency path target")?;
    let target = match target_kind {
        1 => DependencyTarget::Asset(AssetId::from_bytes(target)),
        2 => DependencyTarget::Representation(RepresentationId::from_bytes(target)),
        _ => return Err(stored_invariant("dependency path target kind is invalid")),
    };
    let resolved = resolved
        .map(|value| {
            id_bytes(value, "dependency path resolved representation")
                .map(RepresentationId::from_bytes)
        })
        .transpose()?;
    Ok(ArtifactDependencyPathSegment::new(
        source,
        dependency_position,
        resource,
        kind,
        target,
        resolved,
        authored,
    ))
}

fn captured_issue(status: i64) -> Result<Option<ArtifactDependencyIssue>> {
    match status {
        0 => Ok(None),
        1 => Ok(Some(ArtifactDependencyIssue::NeedsExtraction)),
        2 => Ok(Some(ArtifactDependencyIssue::Unresolved)),
        3 => Ok(Some(ArtifactDependencyIssue::DepthTruncated)),
        4 => Ok(Some(ArtifactDependencyIssue::RepresentationsTruncated)),
        _ => Err(stored_invariant(
            "dependency snapshot path status is invalid",
        )),
    }
}

fn representation_fingerprint_dirty(
    production: &SqliteProduction,
    representation_id: RepresentationId,
) -> Result<bool> {
    production
        .connection
        .query_row(
            "SELECT EXISTS (
                SELECT 1 FROM representation_fingerprint_recomputations
                WHERE representation_id = ?1
             )",
            [representation_id.as_bytes().as_slice()],
            |row| row.get(0),
        )
        .map_err(sqlite_error("read dependency fingerprint marker"))
}

fn current_fingerprints(representation: &Representation) -> Fingerprints {
    representation
        .fingerprints()
        .iter()
        .map(|value| {
            (
                (value.algorithm().to_owned(), value.version()),
                value.value().to_vec(),
            )
        })
        .collect()
}

fn stored_invariant(message: &'static str) -> Error {
    Error::new(ErrorKind::Storage, message)
}

fn stored_domain_error(context: &'static str) -> impl FnOnce(Error) -> Error {
    move |error| {
        Error::new(
            ErrorKind::Storage,
            format!("stored {context} is invalid: {error}"),
        )
    }
}

fn promote_state(current: &mut ArtifactKnowledgeState, candidate: ArtifactKnowledgeState) {
    let priority = |state| match state {
        ArtifactKnowledgeState::Current => 0,
        ArtifactKnowledgeState::Stale => 2,
        ArtifactKnowledgeState::Diverged => 3,
        _ => 1,
    };
    if priority(candidate) > priority(*current) {
        *current = candidate;
    }
}
