#include <postproject/postproject.hpp>

#include <set>
#include <type_traits>
#include <unordered_set>

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

int main() {
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
  const postproject::ObjectRef wrong{postproject::ObjectKind::resource, asset.asUuid()};
  const auto rejected = wrong.assetId();
  if (rejected || rejected.error().code() != postproject::ErrorCode::invalid_argument) return 13;
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
