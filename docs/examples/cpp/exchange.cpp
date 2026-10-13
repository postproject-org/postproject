#include <postproject/postproject.hpp>
#include <iostream>
#include <type_traits>

using namespace postproject;
static_assert(!std::is_convertible_v<ClientId, RequestId>);
static_assert(!std::is_copy_constructible_v<Proposal>);
static_assert(std::is_nothrow_move_constructible_v<Proposal>);
static_assert(!std::is_copy_constructible_v<SubmissionResult>);
static_assert(std::is_copy_constructible_v<SubmissionOutcome>);

static Result<void> transport_boundaries() {
  POSTPROJECT_TRY_ASSIGN(auto client, ClientId::fromString(
      "00000000-0000-0000-0000-000000000001"));
  POSTPROJECT_TRY_ASSIGN(auto text, client.toString());
  POSTPROJECT_TRY_ASSIGN(auto request, RequestId::fromString(text));
  POSTPROJECT_TRY_ASSIGN(auto request_text, request.toString());
  POSTPROJECT_TRY_ASSIGN(auto history, HistoryId::fromString(request_text));
  POSTPROJECT_TRY_ASSIGN(auto history_text, history.toString());
  if (history_text != text || history.asUuid() != request.asUuid() ||
      history.bytes() != client.bytes())
    return Error(ErrorCode::internal, "explicit exchange ID interchange");
  const auto malformed = Proposal::parse({'{', '}'});
  if (malformed || malformed.error().code() != ErrorCode::malformed)
    return Error(ErrorCode::internal, "malformed proposal category");
  return {};
}

static Result<void> prepare(const std::string &path) {
  POSTPROJECT_TRY_ASSIGN(auto production, Production::create(path));
  // [exchange-checkpoint]
  const std::filesystem::path checkpoint = path + ".checkpoint";
  const std::filesystem::path mirror_path = path + ".mirror.pproj";
  POSTPROJECT_TRY_ASSIGN(auto saved, production.exportCheckpoint(checkpoint));
  POSTPROJECT_TRY_ASSIGN(auto validated, Production::validateCheckpoint(checkpoint));
  POSTPROJECT_TRY_ASSIGN(auto mirror, Production::importCheckpoint(checkpoint, mirror_path));
  POSTPROJECT_TRY_ASSIGN(auto state, mirror.exchangeState());
  if (saved.digest != validated.digest || state.role != ExchangeRole::passive_mirror ||
      !state.head || state.head->digest != saved.digest || state.protocol_version != 1 ||
      state.supported_features != UINT64_C(511))
    return Error(ErrorCode::internal, "checkpoint source boundary");
  // [/exchange-checkpoint]
  if (mirror.edit(state.observed_base.decision).error().code() != ErrorCode::unsupported)
    return Error(ErrorCode::internal, "passive ordinary edit rejection");
  auto tiny = state.limits;
  tiny.file_bytes = 1;
  if (Production::validateCheckpoint(checkpoint, tiny).error().code() != ErrorCode::limit_exceeded ||
      production.exportCheckpoint(checkpoint).error().code() != ErrorCode::already_exists)
    return Error(ErrorCode::internal, "bounded exclusive checkpoint publication");
  // [exchange-prepare]
  POSTPROJECT_TRY_ASSIGN(auto view, production.readSession());
  POSTPROJECT_TRY_ASSIGN(auto base, view.decisionBase());
  POSTPROJECT_TRY_ASSIGN(auto edit, production.edit(base));
  POSTPROJECT_TRY(edit.addMetadataValue(ObjectRef::production(base.production_id),
      "example:Exact", "count", MetadataValue::unsignedInteger(UINT64_MAX)));
  const ClientId client(Uuid::Bytes{1});
  const RequestId request(Uuid::Bytes{2});
  POSTPROJECT_TRY_ASSIGN(auto proposal, edit.prepareProposal(client, request, state.observed_base));
  POSTPROJECT_TRY_ASSIGN(auto identity, proposal.identity());
  POSTPROJECT_TRY_ASSIGN(auto bytes, proposal.document());
  POSTPROJECT_TRY_ASSIGN(auto transported, Proposal::parse(bytes));
  POSTPROJECT_TRY_ASSIGN(auto copied, transported.identity());
  POSTPROJECT_TRY_ASSIGN(auto head, production.latestRevision());
  if (head || copied.production_id != base.production_id ||
      identity.history_id != copied.history_id || copied.request_id != request ||
      copied.client_id != client || edit.commit().has_value())
    return Error(ErrorCode::internal, "prepared intent identity or lifecycle");
  // [/exchange-prepare]
  // [exchange-submit]
  POSTPROJECT_TRY_ASSIGN(auto submission, production.submitProposal(transported));
  const auto *receipt = std::get_if<CommitReceipt>(&submission.outcome.status);
  if (!receipt || !receipt->revision || receipt->revision->sequence != 1 ||
      !submission.leases.empty() || !submission.outcome.jobs.empty())
    return Error(ErrorCode::internal, "own submitted receipt");
  POSTPROJECT_TRY_ASSIGN(production, Production::open(path));
  POSTPROJECT_TRY_ASSIGN(auto recovered, production.submissionOutcome(identity));
  POSTPROJECT_TRY_ASSIGN(auto duplicate, production.submitProposal(proposal));
  if (recovered.document != submission.outcome.document ||
      duplicate.outcome.document != recovered.document || !duplicate.leases.empty())
    return Error(ErrorCode::internal, "lost reply and duplicate recovery");
  // [/exchange-submit]
  // [exchange-catch-up]
  const std::filesystem::path changes = path + ".changes";
  POSTPROJECT_TRY_ASSIGN(auto through, production.exportChanges(saved, changes));
  POSTPROJECT_TRY_ASSIGN(auto applied, mirror.applyChanges(changes));
  if (through.digest != applied.digest || !applied.base.decision.revision ||
      applied.base.decision.revision->sequence != 1)
    return Error(ErrorCode::internal, "complete catch-up boundary");
  POSTPROJECT_TRY_ASSIGN(mirror, Production::open(path + ".mirror.pproj"));
  POSTPROJECT_TRY_ASSIGN(applied, mirror.applyChanges(changes));
  POSTPROJECT_TRY_ASSIGN(state, mirror.exchangeState());
  if (!state.head || applied.digest != state.head->digest)
    return Error(ErrorCode::internal, "restart and duplicate catch-up");
  POSTPROJECT_TRY_ASSIGN(auto exact, mirror.queryMetadata("example:Exact", "count", 10));
  if (exact.items.size() != 1 ||
      exact.items[0].value.getIf<MetadataUnsignedInteger>()->value != UINT64_MAX)
    return Error(ErrorCode::internal, "ordinary exact metadata read");
  // [/exchange-catch-up]
  if (production.applyChanges(changes).error().code() != ErrorCode::mirror_read_only ||
      mirror.resynchronizeHistory(state.observed_base, 100, 1048576).error().code() !=
          ErrorCode::mirror_read_only)
    return Error(ErrorCode::internal, "exchange authority separation");
  POSTPROJECT_TRY_ASSIGN(auto authority_state, production.exchangeState());
  POSTPROJECT_TRY_ASSIGN(auto floor,
      production.resynchronizeHistory(authority_state.observed_base, 100, 1048576));
  if (floor.digest != saved.digest)
    return Error(ErrorCode::internal, "complete floor remains stable");
  return {};
}

static Result<void> rejected_intent(const std::string &path) {
  POSTPROJECT_TRY_ASSIGN(auto production, Production::open(path));
  POSTPROJECT_TRY_ASSIGN(auto state, production.exchangeState());
  POSTPROJECT_TRY_ASSIGN(auto edit, production.edit(state.observed_base.decision));
  const auto target = ObjectRef::production(state.observed_base.decision.production_id);
  POSTPROJECT_TRY(edit.removeMetadataProperty(target, "example:Exact", "count"));
  POSTPROJECT_TRY_ASSIGN(auto proposal, edit.prepareProposal(ClientId(Uuid::Bytes{1}),
      RequestId(Uuid::Bytes{3}), state.observed_base));
  POSTPROJECT_TRY_ASSIGN(auto later, production.edit(state.observed_base.decision));
  POSTPROJECT_TRY(later.addMetadataValue(target, "example:Exact", "count",
      MetadataValue::unsignedInteger(7)));
  POSTPROJECT_TRY(later.commit());
  POSTPROJECT_TRY_ASSIGN(auto submitted, production.submitProposal(proposal));
  const auto *rejection = std::get_if<Error>(&submitted.outcome.status);
  if (!rejection || rejection->code() != ErrorCode::conflict || !rejection->transactionConflict())
    return Error(ErrorCode::internal, "retained structured conflict");
  POSTPROJECT_TRY_ASSIGN(auto identity, proposal.identity());
  POSTPROJECT_TRY_ASSIGN(production, Production::open(path));
  POSTPROJECT_TRY_ASSIGN(auto recovered, production.submissionOutcome(identity));
  if (recovered.document != submitted.outcome.document ||
      !std::get<Error>(recovered.status).transactionConflict())
    return Error(ErrorCode::internal, "reopened rejection detail");
#if POSTPROJECT_HAS_EXCEPTIONS
  try {
    production.edit(state.observed_base.decision).value().prepareProposal(
        identity.client_id, identity.request_id,
        ProtocolBase{state.observed_base.decision, HistoryId(Uuid::Bytes{9})}).value();
    return Error(ErrorCode::internal, "foreign scope should throw");
  } catch (const Exception &error) {
    if (error.code() != ErrorCode::scope_mismatch)
      return Error(ErrorCode::internal, "typed scope exception");
  }
#endif
  return {};
}

static Result<void> worker_delivery(const std::string &path, const std::string &media) {
  POSTPROJECT_TRY_ASSIGN(auto production, Production::create(path));
  POSTPROJECT_TRY_ASSIGN(auto setup, production.beginTransaction());
  POSTPROJECT_TRY_ASSIGN(auto imported, setup.importMedia(media));
  const auto asset = imported.asset_id;
  POSTPROJECT_TRY(setup.commit());
  POSTPROJECT_TRY_ASSIGN(auto request, production.beginTransaction());
  POSTPROJECT_TRY_ASSIGN(auto job, request.requestJob({"example:proxy", {imported.representation_id},
      asset, RepresentationKind::proxy, std::nullopt}));
  POSTPROJECT_TRY(request.commit());
  const ClientId client(Uuid::Bytes{1});
  POSTPROJECT_TRY_ASSIGN(auto claim, production.beginTransaction());
  POSTPROJECT_TRY_ASSIGN(auto pending, claim.claimJobLease(job,
      {"Worker", std::nullopt, std::nullopt}, std::chrono::minutes(1)));
  POSTPROJECT_TRY_ASSIGN(auto proposal, claim.prepareProposal(client, RequestId(Uuid::Bytes{1})));
  POSTPROJECT_TRY_ASSIGN(auto closed, pending.state());
  if (!std::holds_alternative<ClosedJobLease>(closed))
    return Error(ErrorCode::internal, "prepared claim closes pending ownership");
  POSTPROJECT_TRY_ASSIGN(auto delivery, production.submitProposal(proposal));
  if (delivery.leases.size() != 1 || delivery.outcome.jobs.size() != 1 ||
      delivery.outcome.jobs[0].job_id != job ||
      !std::holds_alternative<ClaimedJobResult>(delivery.outcome.jobs[0].status))
    return Error(ErrorCode::internal, "fresh lease and public observation");
  auto lease = std::move(delivery.leases[0]);
  POSTPROJECT_TRY_ASSIGN(auto renew, production.beginTransaction());
  POSTPROJECT_TRY(renew.renewJobLease(lease, std::chrono::minutes(2)));
  POSTPROJECT_TRY_ASSIGN(auto renewal, renew.prepareProposal(client, RequestId(Uuid::Bytes{2})));
  POSTPROJECT_TRY_ASSIGN(auto renewed, production.submitProposal(renewal, {std::cref(lease)}));
  POSTPROJECT_TRY_ASSIGN(auto release, production.beginTransaction());
  POSTPROJECT_TRY(release.releaseJobLease(lease));
  POSTPROJECT_TRY(release.commit());
  POSTPROJECT_TRY_ASSIGN(production, Production::open(path));
  POSTPROJECT_TRY_ASSIGN(auto duplicate, production.submitProposal(renewal, {std::cref(lease)}));
  if (!duplicate.leases.empty() || duplicate.outcome.document != renewed.outcome.document)
    return Error(ErrorCode::internal, "private recovery precedes current ownership");
  if (production.submitProposal(renewal).error().code() != ErrorCode::request_identity_mismatch)
    return Error(ErrorCode::internal, "private binding is required for retries");
  POSTPROJECT_TRY_ASSIGN(auto identity, renewal.identity());
  POSTPROJECT_TRY_ASSIGN(auto public_facts, production.submissionOutcome(identity));
  if (public_facts.document != renewed.outcome.document)
    return Error(ErrorCode::internal, "public recovery without capability");
  return {};
}

int main(int argc, char **argv) {
  if (argc != 2) return 2;
  auto result = transport_boundaries();
  if (result) result = prepare(std::string(argv[1]) + "/cpp-authority.pproj");
  if (result) result = rejected_intent(std::string(argv[1]) + "/cpp-authority.pproj");
  if (result) result = worker_delivery(std::string(argv[1]) + "/cpp-worker.pproj",
      std::string(argv[1]) + "/rushes/A001.mov");
  if (!result) { std::cerr << result.error().message() << '\n'; return 1; }
  return 0;
}
