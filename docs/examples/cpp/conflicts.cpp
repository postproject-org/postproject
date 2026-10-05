// Runs the C++ listing in the semantic-conflicts guide.

#include <postproject/postproject.hpp>

#include <iostream>
#include <stdexcept>
#include <string>

namespace {

void require(bool condition, const char *message) {
  if (!condition) {
    throw std::runtime_error(message);
  }
}

// [semantic-conflicts]
void update_from_a_base_revision(postproject::Production &production) {
  auto setup = production.beginTransaction().value();
  const auto root_id = setup.addMediaRoot("rushes").value();
  setup.commit().value();
  require(postproject::MediaRootId::fromString(root_id.toString().value()).value()
              == root_id, "saved root identity");

  const auto base = production.latestRevision().value().value();
  auto first_writer = production.beginTransaction(base.id).value();
  first_writer.setMediaRootEnabled(root_id, false).value();
  const auto receipt = first_writer.commitWithReceipt().value();
  require(receipt.production_id == production.id().value(), "receipt production");
  require(receipt.revision.has_value(), "created revision");
  const auto superseding = *receipt.revision;
  auto stale_writer = production.beginTransaction(base.id).value();
  stale_writer.setMediaRootEnabled(root_id, true).value();
  const auto result = stale_writer.commit();
  require(!result, "stale write must conflict");
  const auto *conflict = result.error().transactionConflict();
  require(conflict != nullptr, "structured conflict detail");
  require(conflict->key.kind == postproject::ConflictKeyKind::media_root,
          "media-root conflict key");
  const auto *conflicting_root =
      std::get_if<postproject::MediaRootId>(&conflict->key.target);
  require(conflicting_root && *conflicting_root == root_id, "conflicting root");
  require(conflict->base_revision_id == base.id, "supplied base revision");
  require(conflict->superseding_revision_id == superseding.id,
          "superseding revision");

  // Retry only after re-reading and deciding that enabling is still right.
  const auto refreshed = production.latestRevision().value().value();
  auto retry = production.beginTransaction(refreshed.id).value();
  retry.setMediaRootEnabled(root_id, true).value();
  retry.commit().value();
}
// [/semantic-conflicts]

} // namespace

int main(int argc, char **argv) {
  if (argc != 2) {
    std::cerr << "usage: postproject-cpp-conflicts WORK_DIRECTORY\n";
    return 2;
  }
  try {
    auto production = postproject::Production::create(
                          std::string(argv[1]) + "/conflicts.pproj")
                          .value();
    update_from_a_base_revision(production);
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
