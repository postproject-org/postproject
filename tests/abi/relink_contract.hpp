// Runs with exception propagation and with Result propagation without exceptions.
#include <postproject/postproject.hpp>

inline postproject::Result<void> relink_noop_contract(
    postproject::Production &production, const postproject::AssetId &asset,
    const postproject::RepresentationId &representation) {
  postproject::ResolutionConfig config;
  config.verification = postproject::VerificationMode::content;
  POSTPROJECT_TRY_ASSIGN(auto options, postproject::ResolutionOptions::create(config));
  POSTPROJECT_TRY_ASSIGN(auto values, production.resolveAsset(asset, options));
  const auto selected = std::find_if(values.begin(), values.end(),
      [&](const auto &value) { return value.representation_id == representation; });
  if (selected == values.end())
    return postproject::Error(postproject::ErrorCode::internal, "retained representation missing");
  POSTPROJECT_TRY_ASSIGN(auto selection, selected->selectVerified());
  values.clear(); // Copied candidates, evidence and base outlive the result values.
  if (selection.locations.empty() || selection.locations.front().candidate.evidence.empty())
    return postproject::Error(postproject::ErrorCode::internal, "selection lost candidate evidence");
  POSTPROJECT_TRY_ASSIGN(auto edit, production.edit(selection.base));
  auto malformed = selection;
  malformed.locations.front().candidate.uri = std::string("bad\0uri", 7);
  const auto rejected = edit.confirmSelection(malformed);
  if (rejected || rejected.error().code() != postproject::ErrorCode::invalid_argument)
    return postproject::Error(postproject::ErrorCode::internal, "malformed selection accepted");
  POSTPROJECT_TRY(edit.confirmSelection(selection));
  POSTPROJECT_TRY_ASSIGN(const auto receipt, edit.commit());
  if (receipt.revision)
    return postproject::Error(postproject::ErrorCode::internal, "known location was not a no-op");
  return {};
}
