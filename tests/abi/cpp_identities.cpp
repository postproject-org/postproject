#include <postproject/postproject.hpp>

#include <set>
#include <type_traits>
#include <unordered_set>

static_assert(!std::is_convertible_v<postproject::Uuid, postproject::ProductionId>);
static_assert(!std::is_convertible_v<postproject::ProductionId, postproject::Uuid>);
static_assert(!std::is_same_v<postproject::ProductionId, postproject::Uuid>);
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

int main() {
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
