//! Knowledge-only evaluation of activity-produced representations.

use std::collections::{BTreeMap, BTreeSet};

use postproject_core::{
    Activity, ActivityEdgeSnapshot, ArtifactEdgeKind, ArtifactEvaluation, ArtifactEvaluationLimits,
    ArtifactKnowledgeReason, ArtifactKnowledgeState, ArtifactReproducibilityIssue,
    ArtifactReproducibilityReport, ArtifactTraversalLimitKind, Error, ErrorKind, ObjectRef,
    Representation, RepresentationId, Result,
};
use rusqlite::params;

use crate::{
    SqliteProduction, artifact_reasons::Reasons,
    dependency_evaluation::evaluate_input_dependencies, read_budget::ReadBudget, sqlite_error,
};

struct NodeEvaluation {
    state: ArtifactKnowledgeState,
    reasons: Reasons,
}

struct EvaluationContext {
    limits: ArtifactEvaluationLimits,
    visited: BTreeSet<RepresentationId>,
    cache: BTreeMap<RepresentationId, NodeEvaluation>,
    cache_budget: ReadBudget,
    truncated: bool,
}

type FingerprintDomainDifference = (String, u16, Option<Vec<u8>>, Option<Vec<u8>>);

impl SqliteProduction {
    /// Reports whether recorded production knowledge can reproduce an artifact.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::NotFound`] when the target representation is absent,
    /// or a storage-domain error when persisted provenance is malformed.
    pub fn artifact_reproducibility(
        &self,
        representation_id: RepresentationId,
    ) -> Result<ArtifactReproducibilityReport> {
        self.load_representation_by_id(representation_id)?;
        let producers = self.activities_producing(representation_id)?;
        match producers.as_slice() {
            [] => Ok(ArtifactReproducibilityReport::new(
                representation_id,
                None,
                None,
                vec![ArtifactReproducibilityIssue::ProducingActivityMissing],
            )),
            [activity] => self.activity_reproducibility(representation_id, activity),
            activities => Ok(ArtifactReproducibilityReport::new(
                representation_id,
                None,
                None,
                vec![ArtifactReproducibilityIssue::ProducingActivityAmbiguous {
                    activity_count: u32::try_from(activities.len()).unwrap_or(u32::MAX),
                }],
            )),
        }
    }

    fn activity_reproducibility(
        &self,
        representation_id: RepresentationId,
        activity: &Activity,
    ) -> Result<ArtifactReproducibilityReport> {
        let mut issues = Vec::new();
        if activity.tool().is_none() {
            issues.push(ArtifactReproducibilityIssue::ToolIdentityMissing {
                activity_id: activity.id(),
            });
        }
        if self
            .metadata(ObjectRef::Activity(activity.id()))?
            .is_empty()
        {
            issues.push(ArtifactReproducibilityIssue::ParametersMissing {
                activity_id: activity.id(),
            });
        }
        for input in activity.inputs() {
            if let Err(error) = self.load_representation_by_id(input.representation_id()) {
                if error.kind() != ErrorKind::NotFound {
                    return Err(error);
                }
                issues.push(ArtifactReproducibilityIssue::InputRepresentationMissing {
                    activity_id: activity.id(),
                    representation_id: input.representation_id(),
                });
            }
        }
        Ok(ArtifactReproducibilityReport::new(
            representation_id,
            Some(activity.id()),
            Some(activity.kind().clone()),
            issues,
        ))
    }

    /// Evaluates current, stale, indeterminate, or diverged artifact knowledge.
    ///
    /// This operation reads only production knowledge and never accesses media
    /// files or mutates the production.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::NotFound`] when the target representation is absent,
    /// or a storage-domain error when persisted provenance is malformed.
    pub fn evaluate_artifact(
        &self,
        representation_id: RepresentationId,
        limits: ArtifactEvaluationLimits,
    ) -> Result<ArtifactEvaluation> {
        self.load_representation_by_id(representation_id)?;
        let mut context = EvaluationContext {
            limits,
            visited: BTreeSet::new(),
            cache: BTreeMap::new(),
            cache_budget: ReadBudget::default(),
            truncated: false,
        };
        let evaluation = self.evaluate_artifact_node(representation_id, 0, &mut context)?;
        let visited_representations = u32::try_from(context.visited.len()).map_err(|error| {
            Error::new(
                ErrorKind::Storage,
                format!("artifact traversal count is unsupported: {error}"),
            )
        })?;
        Ok(ArtifactEvaluation::new(
            representation_id,
            evaluation.state,
            evaluation.reasons.into_vec(),
            visited_representations,
            context.truncated,
        ))
    }

    fn evaluate_artifact_node(
        &self,
        representation_id: RepresentationId,
        depth: u32,
        context: &mut EvaluationContext,
    ) -> Result<NodeEvaluation> {
        if let Some(evaluation) = context.cache.get(&representation_id) {
            return Ok(NodeEvaluation {
                state: evaluation.state,
                reasons: evaluation
                    .reasons
                    .checked_clone(&mut context.cache_budget)?,
            });
        }
        if depth > context.limits.max_depth() {
            return truncated_evaluation(
                context,
                ArtifactTraversalLimitKind::Depth,
                representation_id,
            );
        }
        if context.visited.len()
            >= usize::try_from(context.limits.max_representations()).unwrap_or(usize::MAX)
        {
            return truncated_evaluation(
                context,
                ArtifactTraversalLimitKind::Representations,
                representation_id,
            );
        }
        context.visited.insert(representation_id);

        let producers = self.activities_producing(representation_id)?;
        let evaluation = match producers.as_slice() {
            [] => NodeEvaluation {
                state: ArtifactKnowledgeState::Indeterminate,
                reasons: Reasons::one(ArtifactKnowledgeReason::ProducingActivityMissing {
                    representation_id,
                })?,
            },
            [activity] => {
                self.evaluate_activity_output(activity, representation_id, depth, context)?
            }
            activities => NodeEvaluation {
                state: ArtifactKnowledgeState::Indeterminate,
                reasons: Reasons::one(ArtifactKnowledgeReason::ProducingActivityAmbiguous {
                    representation_id,
                    activity_count: u32::try_from(activities.len()).unwrap_or(u32::MAX),
                })?,
            },
        };
        context.cache.insert(
            representation_id,
            NodeEvaluation {
                state: evaluation.state,
                reasons: evaluation
                    .reasons
                    .checked_clone(&mut context.cache_budget)?,
            },
        );
        Ok(evaluation)
    }

    fn evaluate_activity_output(
        &self,
        activity: &Activity,
        representation_id: RepresentationId,
        depth: u32,
        context: &mut EvaluationContext,
    ) -> Result<NodeEvaluation> {
        let mut evaluation = NodeEvaluation {
            state: ArtifactKnowledgeState::Current,
            reasons: Reasons::default(),
        };
        let output = activity
            .outputs()
            .iter()
            .find(|edge| edge.representation_id() == representation_id)
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::Storage,
                    "producing activity has no matching output edge",
                )
            })?;
        self.compare_edge(
            activity,
            &self.load_representation_by_id(representation_id)?,
            output.snapshot(),
            ArtifactEdgeKind::Output,
            &mut evaluation,
        )?;

        for input in activity.inputs() {
            let input_id = input.representation_id();
            self.compare_edge(
                activity,
                &self.load_representation_by_id(input_id)?,
                input.snapshot(),
                ArtifactEdgeKind::Input,
                &mut evaluation,
            )?;
            let dependency_evaluation = evaluate_input_dependencies(self, activity, input)?;
            promote_state(&mut evaluation.state, dependency_evaluation.state);
            evaluation.reasons.extend(dependency_evaluation.reasons)?;
            if !self.activities_producing(input_id)?.is_empty() {
                let upstream = self.evaluate_artifact_node(input_id, depth + 1, context)?;
                if upstream.state != ArtifactKnowledgeState::Current {
                    let downstream_state = match upstream.state {
                        ArtifactKnowledgeState::Stale | ArtifactKnowledgeState::Diverged => {
                            ArtifactKnowledgeState::Stale
                        }
                        _ => ArtifactKnowledgeState::Indeterminate,
                    };
                    promote_state(&mut evaluation.state, downstream_state);
                    evaluation
                        .reasons
                        .push(ArtifactKnowledgeReason::UpstreamNotCurrent {
                            representation_id: input_id,
                            state: upstream.state,
                        })?;
                }
                evaluation.reasons.extend(upstream.reasons)?;
            }
        }
        Ok(evaluation)
    }

    fn compare_edge(
        &self,
        activity: &Activity,
        representation: &Representation,
        snapshot: Option<&ActivityEdgeSnapshot>,
        edge: ArtifactEdgeKind,
        evaluation: &mut NodeEvaluation,
    ) -> Result<()> {
        let representation_id = representation.id();
        let Some(snapshot) = snapshot else {
            promote_state(&mut evaluation.state, ArtifactKnowledgeState::Indeterminate);
            evaluation
                .reasons
                .push(ArtifactKnowledgeReason::SnapshotAbsent {
                    activity_id: activity.id(),
                    representation_id,
                    edge,
                })?;
            return Ok(());
        };
        if self.representation_fingerprint_dirty(representation_id)? {
            promote_state(
                &mut evaluation.state,
                if edge == ArtifactEdgeKind::Output {
                    ArtifactKnowledgeState::Diverged
                } else {
                    ArtifactKnowledgeState::Stale
                },
            );
            evaluation
                .reasons
                .push(ArtifactKnowledgeReason::FingerprintRecomputationPending {
                    activity_id: activity.id(),
                    representation_id,
                    edge,
                })?;
            return Ok(());
        }

        let captured = captured_fingerprints(snapshot);
        let current = current_fingerprints(representation);
        let domains = captured
            .keys()
            .chain(current.keys())
            .cloned()
            .collect::<BTreeSet<_>>();
        if domains.is_empty() {
            add_missing_evidence(activity, representation_id, edge, None, evaluation)?;
            return Ok(());
        }
        for (algorithm, version) in domains {
            match (
                captured.get(&(algorithm.clone(), version)),
                current.get(&(algorithm.clone(), version)),
            ) {
                (Some(snapshot_value), Some(current_value)) if snapshot_value != current_value => {
                    promote_state(
                        &mut evaluation.state,
                        if edge == ArtifactEdgeKind::Output {
                            ArtifactKnowledgeState::Diverged
                        } else {
                            ArtifactKnowledgeState::Stale
                        },
                    );
                    evaluation
                        .reasons
                        .push(ArtifactKnowledgeReason::FingerprintChanged {
                            activity_id: activity.id(),
                            representation_id,
                            edge,
                            algorithm,
                            version,
                            snapshot_value: snapshot_value.clone(),
                            current_value: current_value.clone(),
                        })?;
                }
                (Some(_), Some(_)) => {}
                (snapshot_value, current_value) => add_missing_evidence(
                    activity,
                    representation_id,
                    edge,
                    Some((
                        algorithm,
                        version,
                        snapshot_value.cloned(),
                        current_value.cloned(),
                    )),
                    evaluation,
                )?,
            }
        }
        Ok(())
    }

    fn representation_fingerprint_dirty(
        &self,
        representation_id: RepresentationId,
    ) -> Result<bool> {
        self.connection
            .query_row(
                "SELECT EXISTS (
                    SELECT 1 FROM representation_fingerprint_recomputations
                    WHERE representation_id = ?1
                 )",
                params![representation_id.as_bytes().as_slice()],
                |row| row.get(0),
            )
            .map_err(sqlite_error("read representation fingerprint marker"))
    }
}

fn add_missing_evidence(
    activity: &Activity,
    representation_id: RepresentationId,
    edge: ArtifactEdgeKind,
    domain: Option<FingerprintDomainDifference>,
    evaluation: &mut NodeEvaluation,
) -> Result<()> {
    promote_state(&mut evaluation.state, ArtifactKnowledgeState::Indeterminate);
    let (algorithm, version, snapshot_value, current_value) = domain.map_or(
        (None, None, None, None),
        |(algorithm, version, snapshot, current)| {
            (Some(algorithm), Some(version), snapshot, current)
        },
    );
    evaluation
        .reasons
        .push(ArtifactKnowledgeReason::FingerprintEvidenceMissing {
            activity_id: activity.id(),
            representation_id,
            edge,
            algorithm,
            version,
            snapshot_value,
            current_value,
        })?;
    Ok(())
}

fn captured_fingerprints(snapshot: &ActivityEdgeSnapshot) -> BTreeMap<(String, u16), Vec<u8>> {
    snapshot
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

fn current_fingerprints(representation: &Representation) -> BTreeMap<(String, u16), Vec<u8>> {
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

fn truncated_evaluation(
    context: &mut EvaluationContext,
    limit: ArtifactTraversalLimitKind,
    representation_id: RepresentationId,
) -> Result<NodeEvaluation> {
    context.truncated = true;
    Ok(NodeEvaluation {
        state: ArtifactKnowledgeState::Indeterminate,
        reasons: Reasons::one(ArtifactKnowledgeReason::TraversalTruncated {
            limit,
            representation_id,
        })?,
    })
}

fn promote_state(current: &mut ArtifactKnowledgeState, candidate: ArtifactKnowledgeState) {
    if state_priority(candidate) > state_priority(*current) {
        *current = candidate;
    }
}

const fn state_priority(state: ArtifactKnowledgeState) -> u8 {
    match state {
        ArtifactKnowledgeState::Current => 0,
        ArtifactKnowledgeState::Stale => 2,
        ArtifactKnowledgeState::Diverged => 3,
        _ => 1,
    }
}
