// Runs the C++ listings about opening productions, transactions, and errors.
//
// Each "[name]" ... "[/name]" region is included verbatim by the documentation
// build, so keep regions self-contained and readable. Usage:
//   postproject-cpp-lifecycle WORK_DIRECTORY
// The work directory is prepared by prepare-workdir.cmake.
#include <postproject/postproject.hpp>

#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <iostream>
#include <optional>
#include <stdexcept>
#include <string>

namespace {

void require(bool condition, const char *message) {
  if (!condition) {
    throw std::runtime_error(message);
  }
}

postproject::Uuid create_with_asset(const std::string &path,
                                    const std::string &media) {
  auto production = postproject::Production::create(path, "Documentary").value();
  auto transaction = production.beginTransaction().value();
  const auto asset_id = transaction.importMedia(media, "Camera A").value();
  transaction.commit().value();
  return asset_id;
}

// [open-production]
std::string format_uuid(const postproject::Uuid &id) {
  // A Uuid is 16 raw bytes; format it however your host needs.
  std::string text;
  char digits[3];
  for (const auto byte : id.bytes()) {
    std::snprintf(digits, sizeof digits, "%02x", byte);
    text += digits;
  }
  return text;
}

postproject::Production open_production(const std::string &path,
                                        const postproject::Uuid &asset_id) {
  auto production = postproject::Production::open(path).value();
  std::cout << "production " << format_uuid(production.id().value()) << '\n';

  if (production.containsAsset(asset_id).value()) {
    std::cout << "asset " << format_uuid(asset_id) << " is present\n";
  }
  // assets() reads the whole set; use the paged overload for large productions.
  for (const auto &asset : production.assets().value()) {
    std::cout << "asset: " << asset.display_name.value_or("(unnamed)") << '\n';
  }

  if (const auto latest = production.latestRevision().value()) {
    std::cout << "latest revision: " << latest->sequence << '\n';
  }
  return production;
}
// [/open-production]

// [transaction-lifecycle]
void commit_then_roll_back(postproject::Production &production) {
  auto transaction = production.beginTransaction().value();
  transaction.setRevisionContext(
      {postproject::OriginIdentity{"com.example.editor", "0.4.0", std::nullopt},
       "Add the rushes root"}).value();
  transaction.addMediaRoot("rushes", "Camera originals").value();
  transaction.commit().value(); // one durable revision

  auto abandoned = production.beginTransaction().value();
  abandoned.addMediaRoot("renders", "Discarded root").value();
  // Rolling back discards every staged change; no revision is written.
  abandoned.rollback().value();
}
// [/transaction-lifecycle]

// [error-handling]
std::optional<postproject::ErrorCode>
try_open(const std::string &missing_path) {
  // Every fallible operation returns a Result instead of throwing, so this
  // works in code built with -fno-exceptions.
  const auto production = postproject::Production::open(missing_path);
  if (production.has_value()) {
    return std::nullopt;
  }
  // code() is stable to branch on; message() is a diagnostic for people.
  const postproject::Error &error = production.error();
  if (error.code() == postproject::ErrorCode::not_found) {
    std::cerr << "no production at " << missing_path << ": "
              << error.message() << '\n';
  }
  // With exceptions enabled, value() on a failed Result throws
  // postproject::Exception instead; without them it aborts.
  return error.code();
}

// POSTPROJECT_TRY_ASSIGN returns a failure to the caller and otherwise
// declares the value; POSTPROJECT_TRY does the same for Result<void>. The
// enclosing function must return a Result.
postproject::Result<std::size_t> count_assets(const std::string &path) {
  POSTPROJECT_TRY_ASSIGN(const auto production,
                         postproject::Production::open(path));
  // transform() maps a value and passes an error through unchanged.
  return production.assets().transform(
      [](const auto &assets) { return assets.size(); });
}
// [/error-handling]

} // namespace

int main(int argc, char **argv) {
  if (argc != 2) {
    std::cerr << "usage: postproject-cpp-lifecycle WORK_DIRECTORY\n";
    return 2;
  }
  const std::string work = argv[1];
  const std::string path = work + "/lifecycle.pproj";
  const std::string media = work + "/rushes/A001.mov";

  try {
    const auto asset_id = create_with_asset(path, media);

    auto production = open_production(path, asset_id);
    require(production.containsAsset(asset_id).value(), "imported asset exists");
    const auto assets = production.assets().value();
    require(assets.size() == 1 && assets.front().id == asset_id,
            "one asset listed");
    const auto before = production.latestRevision().value();
    require(before.has_value() && before->sequence == 1, "import revision");

    commit_then_roll_back(production);
    const auto after = production.latestRevision().value();
    require(after.has_value() && after->sequence == before->sequence + 1,
            "only the committed transaction wrote a revision");
    require(after->message == std::optional<std::string>("Add the rushes root"),
            "revision context");
    const auto roots = production.mediaRoots().value();
    require(roots.size() == 1 && roots.front().name == "rushes",
            "only the committed media root exists");

    require(try_open(work + "/missing.pproj") ==
                postproject::ErrorCode::not_found,
            "not-found error");
    require(count_assets(path).value() == 1, "counted through a Result");
    require(count_assets(work + "/missing.pproj").error().code() ==
                postproject::ErrorCode::not_found,
            "propagated not-found error");
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
