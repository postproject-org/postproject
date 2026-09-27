// Builds with exceptions disabled and handles every failure through Result.

#include <postproject/postproject.hpp>

#include <cstdio>
#include <string>

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
  if (missing.ok() || missing.error().code() == postproject::ErrorCode::ok ||
      missing.error().message().empty()) {
    return 4;
  }

  auto created = postproject::Production::create(path, "No exceptions");
  if (!created.ok()) {
    return 5;
  }
  postproject::Production production = *std::move(created);
  auto transaction = production.beginTransaction();
  if (!transaction.ok()) {
    return 6;
  }
  const auto asset_id = transaction->importMedia(media_path);
  if (!asset_id.ok()) {
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
  if (rejected.ok()) {
    return 9;
  }
  if (!transaction->commit().ok()) {
    return 10;
  }

  const auto representations = production.representations(*asset_id);
  if (!representations.ok() || representations->size() != 1) {
    return 11;
  }
  postproject::ResolutionOptions options;
  options.addRootMapping("missing", path + ".no-such-directory");
  if (!options.error().has_value()) {
    return 12;
  }
  const auto unresolved = production.resolveAsset(*asset_id, options);
  if (unresolved.ok()) {
    return 13;
  }
  const auto resolved = production.resolveAsset(*asset_id);
  if (!resolved.ok() || resolved->size() != 1 ||
      resolved->front().availability !=
          postproject::RepresentationAvailability::online) {
    return 14;
  }
  std::remove(media_path.c_str());
  return 0;
}
