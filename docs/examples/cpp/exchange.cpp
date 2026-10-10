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
  POSTPROJECT_TRY_ASSIGN(auto view, production.readSession());
  POSTPROJECT_TRY_ASSIGN(auto base, view.decisionBase());
  POSTPROJECT_TRY_ASSIGN(auto edit, production.edit(base));
  POSTPROJECT_TRY(edit.addMetadataValue(ObjectRef::production(base.production_id),
      "example:Exact", "count", MetadataValue::unsignedInteger(UINT64_MAX)));
  const ClientId client(Uuid::Bytes{1});
  const RequestId request(Uuid::Bytes{2});
  POSTPROJECT_TRY_ASSIGN(auto proposal, edit.prepareProposal(client, request));
  POSTPROJECT_TRY_ASSIGN(auto identity, proposal.identity());
  POSTPROJECT_TRY_ASSIGN(auto bytes, proposal.document());
  POSTPROJECT_TRY_ASSIGN(auto transported, Proposal::parse(bytes));
  POSTPROJECT_TRY_ASSIGN(auto copied, transported.identity());
  POSTPROJECT_TRY_ASSIGN(auto head, production.latestRevision());
  if (head || copied.production_id != base.production_id ||
      identity.history_id != copied.history_id || copied.request_id != request ||
      copied.client_id != client || edit.commit().has_value())
    return Error(ErrorCode::internal, "prepared intent identity or lifecycle");
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
