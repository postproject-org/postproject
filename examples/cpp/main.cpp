#include <postproject/postproject.hpp>

#include <iostream>
#include <optional>
#include <string>
#include <utility>

namespace {

constexpr const char *editorial = "https://example.com/ns/editorial/1";

// code() is stable to branch on; message() is diagnostic text for people.
void report(const postproject::Error &error) {
  std::cerr << "operation failed (" << static_cast<unsigned>(error.code())
            << "): " << error.message() << '\n';
}

// Checks every Result explicitly, so it also builds with -fno-exceptions.
std::optional<postproject::Uuid> import_media(const std::string &output,
                                              const std::string &media) {
  auto created = postproject::Production::create(output, "C++ quickstart");
  if (!created.ok()) {
    report(created.error());
    return std::nullopt;
  }
  // Owned handles are move-only; move the production out of its Result.
  postproject::Production production = std::move(*created);

  auto transaction = production.beginTransaction();
  if (!transaction.ok()) {
    report(transaction.error());
    return std::nullopt;
  }
  const auto asset_id = transaction->importMedia(media, "Quickstart media");
  if (!asset_id.ok()) {
    // Returning destroys the open transaction, which discards its work.
    report(asset_id.error());
    return std::nullopt;
  }
  const auto committed = transaction->commit();
  if (!committed.ok()) {
    report(committed.error());
    return std::nullopt;
  }
  return *asset_id;
} // The production handle closes here.

// Invalid input does not fail when it is built; the consuming call reports it.
bool rejects_invalid_metadata(postproject::Production &production,
                              const postproject::Uuid &asset_id) {
  const auto duration = postproject::MetadataInput::decimal("twelve", 2);
  auto transaction = production.beginTransaction();
  if (!transaction.ok()) {
    report(transaction.error());
    return false;
  }
  const postproject::ObjectRef asset{postproject::ObjectKind::asset, asset_id};
  const auto added =
      transaction->addMetadataValue(asset, editorial, "duration", duration);
  return !added.ok() &&
         added.error().code() == postproject::ErrorCode::invalid_argument &&
         duration.error().has_value();
} // Never committed: releasing the transaction rolls it back.

} // namespace

int main(int argc, char **argv) {
  if (argc != 3) {
    std::cerr
        << "usage: postproject-cpp-example OUTPUT_PRODUCTION MEDIA_FILE\n";
    return 2;
  }

  const auto asset_id = import_media(argv[1], argv[2]);
  if (!asset_id.has_value()) {
    return 1;
  }

  // Where exceptions are enabled, value() unwraps a Result and throws
  // postproject::Exception on failure.
  try {
    auto production = postproject::Production::open(argv[1]).value();
    std::cout << "representations: "
              << production.representations(*asset_id).value().size() << '\n';
    if (!rejects_invalid_metadata(production, *asset_id)) {
      std::cerr << "invalid metadata input was accepted\n";
      return 1;
    }
  } catch (const postproject::Exception &error) {
    report(error.error());
    return 1;
  }
}
