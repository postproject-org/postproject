#include <postproject/postproject.hpp>
#include <iostream>
#include <type_traits>

using namespace postproject;
static_assert(!std::is_convertible_v<ClientId, RequestId>);
static_assert(!std::is_copy_constructible_v<Proposal>);
static_assert(std::is_nothrow_move_constructible_v<Proposal>);

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

// [exchange-prepare]
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
  // [exchange-catch-up]
  const std::filesystem::path changes = path + ".changes";
  POSTPROJECT_TRY_ASSIGN(auto through, production.exportChanges(saved, changes));
  POSTPROJECT_TRY_ASSIGN(auto applied, mirror.applyChanges(changes));
  if (through.digest != applied.digest)
    return Error(ErrorCode::internal, "complete catch-up boundary");
  POSTPROJECT_TRY_ASSIGN(mirror, Production::open(mirror_path.u8string()));
  POSTPROJECT_TRY_ASSIGN(applied, mirror.applyChanges(changes));
  POSTPROJECT_TRY_ASSIGN(state, mirror.exchangeState());
  if (!state.head || applied.digest != state.head->digest)
    return Error(ErrorCode::internal, "restart and duplicate catch-up");
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
// [/exchange-prepare]

int main(int argc, char **argv) {
  if (argc != 2) return 2;
  auto result = transport_boundaries();
  if (result) result = prepare(std::string(argv[1]) + "/cpp-authority.pproj");
  if (!result) { std::cerr << result.error().message() << '\n'; return 1; }
  return 0;
}
