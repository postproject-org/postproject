// Builds with exceptions disabled and handles every failure through Result.

#include <postproject/postproject.hpp>

#include <cstddef>
#include <cstdio>
#include <string>

namespace {

// Propagates every failure with the public macros.
postproject::Result<postproject::AssetId> import_media(const std::string &path,
                                                   const std::string &media) {
  POSTPROJECT_TRY_ASSIGN(auto production,
                         postproject::Production::create(path, "Propagation"));
  POSTPROJECT_TRY_ASSIGN(auto transaction, production.edit());
  POSTPROJECT_TRY_ASSIGN(const auto imported, transaction.importMedia(media));
  POSTPROJECT_TRY(transaction.commit());
  return imported.asset_id;
}

postproject::Result<std::size_t> count_assets(const std::string &path) {
  return postproject::Production::open(path).and_then(
      [](const postproject::Production &production) {
        return production.assets().transform(
            [](const auto &assets) { return assets.size(); });
      });
}

} // namespace

int main(int argc, char **argv) {
  if (argc != 2) {
    return 2;
  }
  pp_object_ref_t native_reference{};
  native_reference.kind = 99;
  const auto invalid_reference = postproject::detail::object_ref(native_reference);
  if (invalid_reference || invalid_reference.error().code() != postproject::ErrorCode::unsupported) return 25;
  // Malformed native state data must fail before creating a usable variant.
  pp_job_t native_job{};
  native_job.state = 99;
  const auto unknown_state = postproject::detail::job_status_value(native_job);
  if (unknown_state || unknown_state.error().code() != postproject::ErrorCode::unsupported) return 20;
  native_job.state = PP_JOB_FAILED;
  if (postproject::detail::job_status_value(native_job)) return 21;
  native_job.failure_diagnostic = "";
  if (postproject::detail::job_status_value(native_job)) return 22;
  native_job.state = PP_JOB_CLAIMED;
  if (postproject::detail::job_status_value(native_job)) return 23;
  native_job.claim_tool_name = "worker";
  native_job.claim_agent_identifier_scheme = "custom";
  if (postproject::detail::job_status_value(native_job)) return 24;
  const std::string path(argv[1]);
  std::remove(path.c_str());
  const std::string media_path = path + ".media";
  std::FILE *media = std::fopen(media_path.c_str(), "wb");
  if (media == nullptr) {
    return 3;
  }
  std::fputs("no-exceptions media", media);
  std::fclose(media);

  const auto missing = postproject::Production::open(path + ".missing");
  if (missing.has_value() ||
      missing.error().code() == postproject::ErrorCode::ok ||
      missing.error().message().empty()) {
    return 4;
  }

  auto created = postproject::Production::create(path, "No exceptions");
  if (!created.has_value()) {
    return 5;
  }
  postproject::Production production = *std::move(created);
  auto transaction = production.edit();
  if (!transaction.has_value()) {
    return 6;
  }
  const auto imported_media = transaction->importMedia(media_path);
  if (!imported_media.has_value()) {
    return 7;
  }
  const auto asset_id = imported_media->asset_id;
  // A malformed source is reported by the call that uses it.
  const auto unsequenced = transaction->importMedia(
      postproject::MediaSource::imageSequence(
          {path + ".no-such-directory", {"frame", ".exr", 4}, 1, 1, 1, 24, 1,
           {}}));
  if (unsequenced.has_value() ||
      unsequenced.error().code() == postproject::ErrorCode::ok) {
    return 8;
  }
  // Invalid input is reported by the operation that consumes it.
  const auto invalid = postproject::MetadataValue::decimal("not a number", 2);
  const auto rejected = transaction->addMetadataValue(
      postproject::ObjectRef::asset(asset_id), "com.example", "amount",
      invalid);
  if (rejected.has_value()) {
    return 9;
  }
  const auto target = postproject::ObjectRef::asset(asset_id);
  if (!transaction->addMetadataValue(target, "custom:unknown", "count",
          postproject::MetadataValue::unsignedInteger(7)) ||
      !transaction->addMetadataValue(target, "custom:unknown", "count",
          postproject::MetadataValue::signedInteger(-7))) return 26;
  if (!transaction->commit().has_value()) {
    return 10;
  }
  const auto assertions = production.metadata(target);
  if (!assertions || assertions->size() != 2 ||
      !assertions->front().value.getIf<postproject::MetadataUnsignedInteger>() ||
      !assertions->back().value.getIf<postproject::MetadataSignedInteger>()) return 27;
  auto view = production.readSession();
  if (!view) return 28;
  const auto copied = view->metadata(target);
  if (!copied || copied->size() != 2 ||
      !(copied->front().target == target)) return 29;
  std::optional<postproject::ReadSession> moved_view(std::move(*view));
  const auto closed = view->metadata(target);
  if (closed || closed.error().code() != postproject::ErrorCode::invalid_argument)
    return 30;
  moved_view.reset();
  if (copied->back().value.getIf<postproject::MetadataSignedInteger>()->value != -7)
    return 31;

  const auto representations = production.representations(asset_id);
  if (!representations.has_value() || representations->size() != 1 ||
      representations->front().id != imported_media->representation_id) {
    return 11;
  }
  const auto by_path = production.findKnownMedia(std::filesystem::path(media_path), 1);
  const auto by_evidence = production.findKnownMedia(
      representations->front().resources.front().fingerprints.front(), 1);
  if (!by_path || !by_evidence || by_path->items.size() != 1 ||
      by_path->next_cursor || by_evidence->next_cursor ||
      by_path->items.front().asset_id != asset_id ||
      by_evidence->items.front().representation_id != representations->front().id)
    return 32;
  const auto missing_path = production.findKnownMedia(
      std::filesystem::path(media_path + ".absent"), 1);
  if (missing_path || missing_path.error().code() != postproject::ErrorCode::io)
    return 33;
  auto options_result = postproject::ResolutionOptions::create();
  if (!options_result) return 12;
  auto options = *std::move(options_result);
  postproject::ResolutionConfig config;
  config.root_mappings = {{"missing", path + ".no-such-directory"}};
  const auto invalid_mapping = postproject::ResolutionOptions::create(config);
  if (invalid_mapping || invalid_mapping.error().code() != postproject::ErrorCode::io)
    return 12;
  config.root_mappings.clear();
  config.max_depth = 0;
  const auto invalid_limits = postproject::ResolutionOptions::create(config);
  config.max_depth = 16;
  config.verification = static_cast<postproject::VerificationMode>(99);
  const auto invalid_mode = postproject::ResolutionOptions::create(config);
  config.verification = postproject::VerificationMode::content;
  config.search_directories = {std::string("bad\0path", 8)};
  const auto invalid_directory = postproject::ResolutionOptions::create(config);
  if (invalid_limits || invalid_mode || invalid_directory) return 13;
  // Failed configurations never change the previously validated value.
  if (!production.resolveAsset(asset_id, options)) return 13;
  config.search_directories.clear();
  config.max_entries_per_directory = 1000;
  options_result = postproject::ResolutionOptions::create(config);
  if (!options_result) return 13;
  options = *std::move(options_result);
  auto moved_options = std::move(options);
  if (production.resolveAsset(asset_id, options)) return 13;
  const auto resolved = production.resolveAsset(asset_id, moved_options);
  if (!resolved.has_value() || resolved->size() != 1 ||
      resolved->front().availability !=
          postproject::RepresentationAvailability::online) {
    return 14;
  }
  auto token_result = postproject::CancelToken::create();
  if (!token_result) return 14;
  auto token = *std::move(token_result);
  auto moved_token = std::move(token);
  config.cancel_token = &token;
  if (postproject::ResolutionOptions::create(config)) return 14;
  config.cancel_token = &moved_token;
  auto cancelled_options = postproject::ResolutionOptions::create(config);
  if (!cancelled_options) return 14;
  moved_options = *std::move(cancelled_options);
  token.cancel(); // moved-from token does not affect the retained flag
  moved_token.cancel();
  const auto cancelled = production.resolveAsset(asset_id, moved_options);
  if (cancelled || cancelled.error().code() != postproject::ErrorCode::cancelled) return 14;

  const std::string propagated = path + ".propagated";
  std::remove(propagated.c_str());
  if (!import_media(propagated, media_path).has_value() ||
      count_assets(propagated).value_or(0) != 1) {
    return 15;
  }
  const auto repeated = import_media(propagated, media_path);
  if (repeated.has_value() ||
      repeated.error().code() != postproject::ErrorCode::already_exists) {
    return 16;
  }
  const auto recovered = count_assets(path + ".missing")
                             .or_else([](const postproject::Error &error) {
                               return error.code() ==
                                              postproject::ErrorCode::not_found
                                          ? postproject::Result<std::size_t>(0)
                                          : postproject::Result<std::size_t>(
                                                error);
                             });
  if (!recovered.has_value() || *recovered != 0) {
    return 17;
  }
  std::remove(propagated.c_str());
  std::remove(media_path.c_str());
  return 0;
}
