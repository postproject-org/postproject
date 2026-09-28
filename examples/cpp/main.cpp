#include <postproject/postproject.hpp>

#include <iostream>
#include <string>
#include <utility>

namespace {

constexpr const char *editorial = "https://example.com/ns/editorial/1";

// code() is stable to branch on; message() is diagnostic text for people.
void report(const postproject::Error &error) {
  std::cerr << "operation failed (" << static_cast<unsigned>(error.code())
            << "): " << error.message() << '\n';
}

// Passes every failure on to its caller, one line per call, so it also builds
// with -fno-exceptions. POSTPROJECT_TRY_ASSIGN returns the error of a failed
// Result and otherwise declares the value; POSTPROJECT_TRY does the same for
// Result<void>.
postproject::Result<postproject::Uuid> import_media(const std::string &output,
                                                   const std::string &media) {
  // Owned handles are move-only; the macro moves the value out of its Result.
  POSTPROJECT_TRY_ASSIGN(
      auto production,
      postproject::Production::create(output, "C++ quickstart"));
  POSTPROJECT_TRY_ASSIGN(auto transaction, production.beginTransaction());
  // A failure returns here and destroys the open transaction, which discards
  // its work.
  POSTPROJECT_TRY_ASSIGN(const auto asset_id,
                         transaction.importMedia(media, "Quickstart media"));
  POSTPROJECT_TRY(transaction.commit());
  return asset_id;
} // The production handle closes here.

// Invalid input does not fail when it is built; the consuming call reports it.
bool rejects_invalid_metadata(postproject::Production &production,
                              const postproject::Uuid &asset_id) {
  const auto duration = postproject::MetadataValue::decimal("twelve", 2);
  auto transaction = production.beginTransaction();
  if (!transaction.has_value()) {
    report(transaction.error());
    return false;
  }
  const postproject::ObjectRef asset{postproject::ObjectKind::asset, asset_id};
  const auto added =
      transaction->addMetadataValue(asset, editorial, "duration", duration);
  return !added.has_value() &&
         added.error().code() == postproject::ErrorCode::invalid_argument;
} // Never committed: releasing the transaction rolls it back.

} // namespace

int main(int argc, char **argv) {
  if (argc != 3) {
    std::cerr
        << "usage: postproject-cpp-example OUTPUT_PRODUCTION MEDIA_FILE\n";
    return 2;
  }

  // Check a Result explicitly where the failure is handled.
  const auto asset_id = import_media(argv[1], argv[2]);
  if (!asset_id.has_value()) {
    report(asset_id.error());
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
