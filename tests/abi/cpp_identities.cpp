#include <postproject/postproject.hpp>

#include <set>
#include <limits>
#include <type_traits>
#include <unordered_set>

static_assert(!std::is_aggregate_v<postproject::ObjectRef>);
static_assert(!std::is_aggregate_v<postproject::RepresentationContent>);
static_assert(!std::is_aggregate_v<postproject::ResolutionOutcome>);
static_assert(!std::is_aggregate_v<postproject::AvailabilityIssueDetail>);
static_assert(std::is_same_v<decltype(std::declval<const postproject::AvailabilityIssueDetail &>().value()),
                             const postproject::AvailabilityIssuePayload &>);
static_assert(std::is_same_v<decltype(std::declval<const postproject::ResolutionOutcome &>().value()),
                             const postproject::ResolutionOutcomeValue &>);
static_assert(std::is_same_v<decltype(std::declval<const postproject::ResolutionOutcome &>().candidates()),
                             const std::vector<postproject::ResolutionCandidate> &>);
static_assert(std::is_same_v<decltype(std::declval<const postproject::RepresentationContent &>().value()),
                             const postproject::RepresentationContentValue &>);
static_assert(!std::is_constructible_v<postproject::ObjectRef, postproject::ObjectKind, postproject::Uuid>);
static_assert(!std::is_invocable_v<decltype(&postproject::ObjectRef::asset), postproject::ResourceId>);
static_assert(std::is_same_v<decltype(std::declval<const postproject::ObjectRef &>().value()), const postproject::ObjectRef::Value &>);

static_assert(!std::is_convertible_v<postproject::Uuid, postproject::ProductionId>);
static_assert(!std::is_convertible_v<postproject::ProductionId, postproject::Uuid>);
static_assert(!std::is_same_v<postproject::ProductionId, postproject::Uuid>);
static_assert(!std::is_convertible_v<postproject::Uuid, postproject::AssetId>);
static_assert(!std::is_convertible_v<postproject::AssetId, postproject::Uuid>);
static_assert(!std::is_convertible_v<postproject::Uuid, postproject::MediaRootId>);
static_assert(!std::is_assignable_v<postproject::MediaRootId &, postproject::AssetId>);
static_assert(std::is_same_v<decltype(postproject::MediaRoot::id), postproject::MediaRootId>);
static_assert(std::is_same_v<decltype(postproject::MediaRootAddedEvent::media_root_id), postproject::MediaRootId>);
static_assert(std::is_same_v<decltype(postproject::MediaRootEnabledChangedEvent::media_root_id), postproject::MediaRootId>);
static_assert(std::is_same_v<decltype(postproject::MediaRootRemovedEvent::media_root_id), postproject::MediaRootId>);
static_assert(std::is_same_v<decltype(std::declval<postproject::Transaction &>().addMediaRoot("rushes")), postproject::Result<postproject::MediaRootId>>);
static_assert(!std::is_invocable_v<decltype(&postproject::Transaction::removeMediaRoot), postproject::Transaction &, postproject::AssetId>);
static_assert(!std::is_assignable_v<postproject::AssetId &, postproject::ProductionId>);
static_assert(!std::is_convertible_v<postproject::Uuid, postproject::RevisionId>);
static_assert(!std::is_convertible_v<postproject::RevisionId, postproject::Uuid>);
static_assert(!std::is_convertible_v<postproject::RevisionId, postproject::TransactionId>);
static_assert(!std::is_convertible_v<postproject::ProductionId, postproject::RevisionId>);
static_assert(!std::is_assignable_v<postproject::RevisionId &, postproject::TransactionId>);
static_assert(std::is_same_v<decltype(postproject::Revision::id), postproject::RevisionId>);
static_assert(std::is_same_v<decltype(postproject::Revision::transaction_id), postproject::TransactionId>);
static_assert(!std::is_assignable_v<decltype(postproject::CommittedRevision::id) &, postproject::Uuid>);
static_assert(!std::is_invocable_v<decltype(&pp_production_revision_events),
              const pp_production_t *, pp_transaction_id_t,
              pp_revision_event_set_t **, pp_error_t **>);
static_assert(std::is_same_v<
    decltype(std::declval<const postproject::Production &>().id()),
    postproject::Result<postproject::ProductionId>>);
static_assert(!std::is_assignable_v<decltype(postproject::DecisionBase::production_id) &,
                                    postproject::Uuid>);
static_assert(!std::is_assignable_v<decltype(postproject::HostObjectBinding::production_id) &,
                                    postproject::Uuid>);

static_assert(std::is_same_v<decltype(postproject::Asset::id), postproject::AssetId>);
static_assert(std::is_same_v<decltype(postproject::Representation::asset_id), postproject::AssetId>);
static_assert(std::is_same_v<decltype(postproject::JobRequest::output_asset_id), postproject::AssetId>);
static_assert(std::is_same_v<decltype(std::declval<postproject::Transaction &>().importMedia(
    std::declval<const postproject::MediaSource &>())), postproject::Result<postproject::AssetId>>);
static_assert(!std::is_invocable_v<decltype(&postproject::Production::asset),
    const postproject::Production &, postproject::Uuid>);
static_assert(!std::is_invocable_v<decltype(&postproject::ReadSession::asset),
    const postproject::ReadSession &, postproject::ProductionId>);
static_assert(!std::is_invocable_v<decltype(&pp_production_asset),
    const pp_production_t *, pp_uuid_t, pp_asset_set_t **, pp_error_t **>);

static_assert(!std::is_convertible_v<postproject::Uuid, postproject::LocatorId>);
static_assert(!std::is_convertible_v<postproject::LocatorId, postproject::Uuid>);
static_assert(!std::is_assignable_v<postproject::LocatorId &, postproject::MediaRootId>);
static_assert(std::is_same_v<decltype(postproject::Locator::id), postproject::LocatorId>);
static_assert(std::is_same_v<decltype(postproject::LocatorAddedEvent::locator_id), postproject::LocatorId>);
static_assert(std::is_same_v<decltype(postproject::LocatorRetiredEvent::locator_id), postproject::LocatorId>);
static_assert(!std::is_invocable_v<decltype(&postproject::Transaction::retireLocator),
    postproject::Transaction &, postproject::AssetId>);

static_assert(!std::is_convertible_v<postproject::Uuid, postproject::JobId>);
static_assert(!std::is_convertible_v<postproject::JobId, postproject::Uuid>);
static_assert(!std::is_assignable_v<postproject::JobId &, postproject::AssetId>);
static_assert(std::is_same_v<decltype(postproject::Job::id), postproject::JobId>);
static_assert(std::is_same_v<decltype(postproject::JobRequestedEvent::job_id), postproject::JobId>);
static_assert(std::is_same_v<decltype(postproject::JobSucceededEvent::job_id), postproject::JobId>);
static_assert(std::is_same_v<decltype(std::declval<postproject::Transaction &>().requestJob(
    std::declval<const postproject::JobRequest &>())), postproject::Result<postproject::JobId>>);
static_assert(!std::is_invocable_v<decltype(&postproject::Transaction::cancelJob),
    postproject::Transaction &, postproject::AssetId>);

static_assert(!std::is_convertible_v<postproject::Uuid, postproject::ActivityId>);
static_assert(!std::is_convertible_v<postproject::ActivityId, postproject::Uuid>);
static_assert(!std::is_assignable_v<postproject::ActivityId &, postproject::JobId>);
static_assert(std::is_same_v<decltype(postproject::Activity::id), postproject::ActivityId>);
static_assert(std::is_same_v<decltype(postproject::ActivityCreatedEvent::activity_id), postproject::ActivityId>);
static_assert(std::is_same_v<decltype(postproject::JobCompletion::activity_id), postproject::ActivityId>);
static_assert(std::is_same_v<decltype(std::declval<postproject::Transaction &>().createActivity(
    std::declval<const postproject::ActivitySpec &>())), postproject::Result<postproject::ActivityId>>);
static_assert(!std::is_invocable_v<decltype(&pp_object_ref_from_activity),
    pp_job_id_t, pp_object_ref_t *, pp_error_t **>);

static_assert(!std::is_convertible_v<postproject::Uuid, postproject::RepresentationId>);
static_assert(!std::is_convertible_v<postproject::RepresentationId, postproject::Uuid>);
static_assert(!std::is_assignable_v<postproject::RepresentationId &, postproject::JobId>);
static_assert(std::is_same_v<decltype(postproject::Representation::id), postproject::RepresentationId>);
static_assert(std::is_same_v<decltype(postproject::RepresentationAddedEvent::representation_id), postproject::RepresentationId>);
static_assert(std::is_same_v<decltype(postproject::JobCompletion::representation_id), postproject::RepresentationId>);
static_assert(std::is_same_v<decltype(std::declval<postproject::Transaction &>().addRepresentation(
    std::declval<const postproject::AssetId &>(), postproject::RepresentationKind::proxy,
    std::declval<const postproject::MediaSource &>())), postproject::Result<postproject::RepresentationId>>);
static_assert(!std::is_invocable_v<decltype(&pp_object_ref_from_representation),
    pp_job_id_t, pp_object_ref_t *, pp_error_t **>);

static_assert(!std::is_convertible_v<postproject::Uuid, postproject::ResourceId>);
static_assert(!std::is_convertible_v<postproject::ResourceId, postproject::Uuid>);
static_assert(!std::is_assignable_v<postproject::ResourceId &, postproject::RepresentationId>);
static_assert(std::is_same_v<decltype(postproject::Resource::id), postproject::ResourceId>);
static_assert(std::is_same_v<decltype(postproject::ResourceAddedEvent::resource_id), postproject::ResourceId>);
static_assert(std::is_same_v<decltype(postproject::KnownMediaMatch::resource_id), postproject::ResourceId>);
static_assert(!std::is_invocable_v<decltype(&pp_object_ref_from_resource),
    pp_representation_id_t, pp_object_ref_t *, pp_error_t **>);

int main() {
  using namespace postproject;
  const auto content_resource = ResourceId::fromString("00000000-0000-0000-0000-000000000001").value();
  const std::uint8_t artifact_bytes[] = {1, 2};
  auto copied_bytes = detail::required_artifact_bytes(1, artifact_bytes, 2).value();
  if (copied_bytes != std::vector<std::uint8_t>{1, 2} ||
      detail::required_artifact_bytes(1, nullptr, 2).has_value() ||
      detail::required_artifact_bytes(0, artifact_bytes, 2).has_value() ||
      detail::required_artifact_bytes(1, artifact_bytes, 0).has_value() ||
      detail::required_artifact_bytes(1, artifact_bytes, 16 * 1024 * 1024 + 1).has_value()) return 37;
  auto frame_details = AvailabilityIssueDetail::create(MissingSequenceFrames{{1003, 1001, 1003}}).value();
  if (frame_details.frames() != std::vector<std::int64_t>{1001, 1003} ||
      frame_details.kind() != AvailabilityIssueKind::missing_frames ||
      !std::holds_alternative<MissingSequenceFrames>(frame_details.value())) return 38;
  if (AvailabilityIssueDetail::create(MissingSequenceFrames{{}}).has_value() ||
      AvailabilityIssueDetail::create(MissingSequenceFrames{std::vector<std::int64_t>(100001, 1)}).has_value()) return 39;
  if (!AvailabilityIssueDetail::create(OfflineResourceIssue{}).value().frames().empty() ||
      !AvailabilityIssueDetail::create(AmbiguousResourceIssue{}).value().frames().empty() ||
      !AvailabilityIssueDetail::create(ResourceErrorIssue{}).value().frames().empty()) return 40;
  ResolutionCandidate candidate{"file:///a.mov", 10000, std::nullopt, std::nullopt,
                                {{EvidenceKind::full_hash_match, std::nullopt}}};
  auto candidates = std::vector<ResolutionCandidate>{candidate};
  auto resolved = ResolutionOutcome::create(ResourceResolvedExact{candidates}).value();
  candidates.clear();
  if (resolved.state() != ResourceResolutionState::resolved_exact ||
      resolved.candidates().size() != 1 ||
      !std::holds_alternative<ResourceResolvedExact>(resolved.value())) return 31;
  if (ResolutionOutcome::create(ResourceResolvedExact{{}}).has_value() ||
      ResolutionOutcome::create(ResourceResolvedProbable{{candidate, candidate}}).has_value() ||
      ResolutionOutcome::create(ResourceAmbiguous{{candidate}}).has_value()) return 32;
  const auto ambiguous = ResolutionOutcome::create(ResourceAmbiguous{{candidate, candidate}}).value();
  if (ambiguous.candidates().size() != 2 ||
      !ResolutionOutcome::create(ResourceOffline{}).value().candidates().empty() ||
      !ResolutionOutcome::create(ResourceResolutionFailure{}).value().candidates().empty()) return 33;
  candidate.confidence_basis_points = 10001;
  if (ResolutionOutcome::create(ResourceResolvedExact{{candidate}}).has_value()) return 34;
  candidate.confidence_basis_points = 10000;
  candidate.evidence.clear();
  if (ResolutionOutcome::create(ResourceResolvedExact{{candidate}}).has_value()) return 35;
  candidate.evidence = {{static_cast<EvidenceKind>(999), std::nullopt}};
  if (ResolutionOutcome::create(ResourceResolvedExact{{candidate}}).has_value()) return 36;
  const RepresentationMember required{content_resource, "example:essence", true};
  auto members = std::vector<RepresentationMember>{required};
  auto checked = RepresentationContent::create(PackageContent{members}).value();
  members.clear();
  if (checked.members().size() != 1 || checked.imageSequence() != nullptr ||
      checked.structureKind() != ContentStructureKind::package) return 30;
  auto copy = checked;
  checked = RepresentationContent::create(SingleResourceContent{content_resource}).value();
  if (copy.structureKind() != ContentStructureKind::package ||
      checked.structureKind() != ContentStructureKind::single_resource) return 31;
  const RepresentationContentValue invalid_content[] = {
      PackageContent{{}}, OrderedPartsContent{{}}, PackageContent{{required, required}},
      PackageContent{{{content_resource, "example:sidecar", false}}},
      OrderedPartsContent{{{content_resource, "example:sidecar", false}}},
      PackageContent{{{content_resource, std::nullopt, true}}},
      PackageContent{{{content_resource, "unqualified", true}}},
      ImageSequenceContent{content_resource, {1, 4, 2, 24, 1, {}}},
      ImageSequenceContent{content_resource, {1, 5, 0, 24, 1, {}}},
      ImageSequenceContent{content_resource, {1, 5, 2, 24, 0, {}}},
      ImageSequenceContent{content_resource, {1, 5, 2, 24, 1, {2}}},
  };
  for (const auto &value : invalid_content) {
    const auto rejected = RepresentationContent::create(value);
    if (rejected || rejected.error().code() != ErrorCode::invalid_argument) return 32;
  }
  const auto sequence = RepresentationContent::create(ImageSequenceContent{
      content_resource, {1, 5, 2, 24000, 1001, {5, 1, 5}}}).value();
  if (sequence.imageSequence()->missing_frames != std::vector<std::int64_t>{1, 5} ||
      !std::get_if<ImageSequenceContent>(&sequence.value())) return 33;
  const auto full_domain = RepresentationContent::create(ImageSequenceContent{
      content_resource, {std::numeric_limits<std::int64_t>::min(),
                         std::numeric_limits<std::int64_t>::max(), 1, 24, 1, {0}}});
  if (!full_domain) return 34;
  const ArtifactFingerprintChanged changed_artifact{
      ActivityId(content_resource.asUuid()), RepresentationId(content_resource.asUuid()),
      ArtifactEdgeKind::input, "foreign_digest", 7, {1}, {2}};
  auto artifact_value = ArtifactReasonValue::create(changed_artifact).value();
  if (artifact_value.kind() != ArtifactReasonKind::fingerprint_changed ||
      std::get<ArtifactFingerprintChanged>(artifact_value.value()).current_value !=
          std::vector<std::uint8_t>{2}) return 37;
  auto invalid_artifact = changed_artifact;
  invalid_artifact.current_value.clear();
  if (ArtifactReasonValue::create(invalid_artifact)) return 38;
  invalid_artifact = changed_artifact;
  invalid_artifact.edge_kind = static_cast<ArtifactEdgeKind>(999);
  if (ArtifactReasonValue::create(invalid_artifact)) return 39;
  if (ArtifactReasonValue::create(ArtifactProducerAmbiguous{
          RepresentationId(content_resource.asUuid()), 1})) return 40;
  const auto invalid_producer_count = ReproducibilityProducerAmbiguous::create(1);
  if (invalid_producer_count || invalid_producer_count.error().code() != ErrorCode::invalid_argument) return 35;
  const ArtifactReproducibilityIssue ambiguous_producers = ReproducibilityProducerAmbiguous::create(2).value();
  if (artifactReproducibilityIssueKind(ambiguous_producers) !=
          ArtifactReproducibilityIssueKind::producing_activity_ambiguous ||
      std::get<ReproducibilityProducerAmbiguous>(ambiguous_producers).activityCount() != 2 ||
      std::get_if<ReproducibilityInputMissing>(&ambiguous_producers) != nullptr) return 36;
  const auto resource = postproject::ResourceId::fromString("00000000-0000-0000-0000-000000000001").value();
  const std::set<postproject::ResourceId> resources{resource, resource};
  const std::unordered_set<postproject::ResourceId> resource_hashes{resource, resource};
  if (resources.size() != 1 || resource_hashes.size() != 1 ||
      postproject::ObjectRef::resource(resource).resourceId().value() != resource ||
      resource.toString().value() != "00000000-0000-0000-0000-000000000001") return 30;
  if (postproject::ObjectRef::job(postproject::JobId(resource.asUuid())).resourceId()) return 31;
  if (postproject::ResourceId::fromString("broken") ||
      postproject::ResourceId::fromString(std::string_view("id\0tail", 7))) return 32;
  if (!postproject::ResourceId::fromString("00000000-0000-0000-0000-000000000000")) return 33;

  const auto representation = postproject::RepresentationId::fromString("00000000-0000-0000-0000-000000000001").value();
  const std::set<postproject::RepresentationId> representations{representation, representation};
  const std::unordered_set<postproject::RepresentationId> representation_hashes{representation, representation};
  if (representations.size() != 1 || representation_hashes.size() != 1 ||
      postproject::ObjectRef::representation(representation).representationId().value() != representation ||
      representation.toString().value() != "00000000-0000-0000-0000-000000000001") return 26;
  if (postproject::ObjectRef::job(postproject::JobId(representation.asUuid())).representationId()) return 27;
  if (postproject::RepresentationId::fromString("broken") ||
      postproject::RepresentationId::fromString(std::string_view("id\0tail", 7))) return 28;
  if (!postproject::RepresentationId::fromString("00000000-0000-0000-0000-000000000000")) return 29;

  const auto activity = postproject::ActivityId::fromString("00000000-0000-0000-0000-000000000001").value();
  const std::set<postproject::ActivityId> activities{activity, activity};
  const std::unordered_set<postproject::ActivityId> activity_hashes{activity, activity};
  if (activities.size() != 1 || activity_hashes.size() != 1 ||
      postproject::ObjectRef::activity(activity).activityId().value() != activity ||
      activity.toString().value() != "00000000-0000-0000-0000-000000000001") return 22;
  if (postproject::ObjectRef::job(postproject::JobId(activity.asUuid())).activityId()) return 23;
  if (postproject::ActivityId::fromString("broken") ||
      postproject::ActivityId::fromString(std::string_view("id\0tail", 7))) return 24;
  if (!postproject::ActivityId::fromString("00000000-0000-0000-0000-000000000000")) return 25;

  const auto job = postproject::JobId::fromString("00000000-0000-0000-0000-000000000001").value();
  const std::set<postproject::JobId> jobs{job, job};
  const std::unordered_set<postproject::JobId> job_hashes{job, job};
  if (jobs.size() != 1 || job_hashes.size() != 1 ||
      postproject::ObjectRef::job(job).jobId().value() != job ||
      job.toString().value() != "00000000-0000-0000-0000-000000000001") return 18;
  if (postproject::ObjectRef::asset(postproject::AssetId(job.asUuid())).jobId()) return 19;
  if (postproject::JobId::fromString("broken") ||
      postproject::JobId::fromString(std::string_view("id\0tail", 7))) return 20;
  if (!postproject::JobId::fromString("00000000-0000-0000-0000-000000000000")) return 21;

  const auto locator = postproject::LocatorId::fromString("00000000-0000-0000-0000-000000000001").value();
  const std::set<postproject::LocatorId> locators{locator, locator};
  const std::unordered_set<postproject::LocatorId> locator_hashes{locator, locator};
  if (locators.size() != 1 || locator_hashes.size() != 1 ||
      locator.toString().value() != "00000000-0000-0000-0000-000000000001" ||
      postproject::LocatorId(locator.asUuid()) != locator) return 15;
  if (postproject::LocatorId::fromString("broken") ||
      postproject::LocatorId::fromString(std::string_view("id\0tail", 7))) return 16;
  if (!postproject::LocatorId::fromString("00000000-0000-0000-0000-000000000000")) return 17;

  const auto root = postproject::MediaRootId::fromString("00000000-0000-0000-0000-000000000001").value();
  const std::set<postproject::MediaRootId> roots{root, root};
  const std::unordered_set<postproject::MediaRootId> root_hashes{root, root};
  if (roots.size() != 1 || root_hashes.size() != 1 ||
      root.toString().value() != "00000000-0000-0000-0000-000000000001" ||
      postproject::MediaRootId(root.asUuid()) != root) return 12;
  if (postproject::MediaRootId::fromString("broken") ||
      postproject::MediaRootId::fromString(std::string_view("id\0tail", 7))) return 13;
  if (!postproject::MediaRootId::fromString("00000000-0000-0000-0000-000000000000")) return 14;
  const auto asset = postproject::AssetId::fromString("00000000-0000-0000-0000-000000000001").value();
  const auto target = postproject::ObjectRef::asset(asset);
  if (target.assetId().value() != asset) return 12;
  const auto wrong = postproject::ObjectRef::resource(postproject::ResourceId(asset.asUuid()));
  const auto rejected = wrong.assetId();
  if (rejected || rejected.error().code() != postproject::ErrorCode::invalid_argument) return 13;
  const auto invalid_kind = postproject::ObjectRef::fromUuid(static_cast<postproject::ObjectKind>(99), asset.asUuid());
  if (invalid_kind || invalid_kind.error().code() != postproject::ErrorCode::invalid_argument) return 14;
  if (!std::get_if<postproject::AssetId>(&target.value()) || std::get_if<postproject::ResourceId>(&target.value())) return 15;
  const std::set<postproject::AssetId> assets{asset, asset};
  const std::unordered_set<postproject::AssetId> asset_hashes{asset, asset};
  if (assets.size() != 1 || asset_hashes.size() != 1 ||
      postproject::AssetId(asset.asUuid()) != asset ||
      postproject::AssetId(asset.bytes()) != asset) return 10;
  if (asset.toString().value() != "00000000-0000-0000-0000-000000000001" ||
      postproject::AssetId::fromString("broken").has_value() ||
      postproject::AssetId::fromString(std::string_view("id\0tail", 7)).has_value()) return 11;
  const auto revision = postproject::RevisionId::fromString("00000000-0000-0000-0000-000000000001").value();
  const auto transaction = postproject::TransactionId::fromString("00000000-0000-0000-0000-000000000001").value();
  const std::set<postproject::RevisionId> revisions{revision, revision};
  const std::unordered_set<postproject::TransactionId> transactions{transaction, transaction};
  if (revisions.size() != 1 || transactions.size() != 1 ||
      revision.toString().value() != transaction.toString().value()) return 7;
  if (postproject::RevisionId(revision.asUuid()) != revision ||
      postproject::TransactionId(transaction.bytes()) != transaction) return 8;
  if (postproject::RevisionId::fromString("broken").has_value() ||
      postproject::TransactionId::fromString(std::string_view("valid\0tail", 10)).has_value()) return 9;
  auto parsed = postproject::ProductionId::fromString(
      "12345678-1234-5678-9abc-123456789abc");
  if (!parsed.has_value()) return 1;
  const auto id = *parsed;
  const std::set<postproject::ProductionId> ordered{id, id};
  const std::unordered_set<postproject::ProductionId> hashed{id, id};
  if (ordered.size() != 1 || hashed.size() != 1) return 2;
  if (postproject::ProductionId(id.asUuid()) != id) return 3;
  const auto text = id.toString();
  if (!text.has_value() || *text != "12345678-1234-5678-9abc-123456789abc") return 4;
  const auto invalid = postproject::ProductionId::fromString("broken");
  if (invalid.has_value() || invalid.error().code() != postproject::ErrorCode::invalid_argument) return 5;
  const auto embedded = postproject::ProductionId::fromString(std::string_view("valid\0tail", 10));
  if (embedded.has_value() || embedded.error().code() != postproject::ErrorCode::invalid_argument) return 6;
  return 0;
}
