// Builds with exceptions disabled and handles every failure through Result.

#include <postproject/postproject.hpp>

#include <cstddef>
#include <cstdio>
#include <string>

namespace {

// Propagates every failure with the public macros.
postproject::Result<postproject::Uuid> import_media(const std::string &path,
                                                   const std::string &media) {
  POSTPROJECT_TRY_ASSIGN(auto production,
                         postproject::Production::create(path, "Propagation"));
  POSTPROJECT_TRY_ASSIGN(auto transaction, production.beginTransaction());
  POSTPROJECT_TRY_ASSIGN(const auto asset_id, transaction.importMedia(media));
  POSTPROJECT_TRY(transaction.commit());
  return asset_id;
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
  auto transaction = production.beginTransaction();
  if (!transaction.has_value()) {
    return 6;
  }
  const auto asset_id = transaction->importMedia(media_path);
  if (!asset_id.has_value()) {
    return 7;
  }
  // Invalid input is reported by the operation that consumes it.
  const auto invalid = postproject::MetadataInput::decimal("not a number", 2);
  if (!invalid.error().has_value()) {
    return 8;
  }
  const auto rejected = transaction->addMetadataValue(
      {postproject::ObjectKind::asset, *asset_id}, "com.example", "amount",
      invalid);
  if (rejected.has_value()) {
    return 9;
  }
  if (!transaction->commit().has_value()) {
    return 10;
  }

  const auto representations = production.representations(*asset_id);
  if (!representations.has_value() || representations->size() != 1) {
    return 11;
  }
  postproject::ResolutionOptions options;
  options.addRootMapping("missing", path + ".no-such-directory");
  if (!options.error().has_value()) {
    return 12;
  }
  const auto unresolved = production.resolveAsset(*asset_id, options);
  if (unresolved.has_value()) {
    return 13;
  }
  const auto resolved = production.resolveAsset(*asset_id);
  if (!resolved.has_value() || resolved->size() != 1 ||
      resolved->front().availability !=
          postproject::RepresentationAvailability::online) {
    return 14;
  }

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
