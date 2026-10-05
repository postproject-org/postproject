#include <postproject/postproject.hpp>
#include <stdexcept>
#include <string>
#include <iostream>

// [coherent-reads]
static void exercise(const std::string &path, const std::string &media) {
  auto production = postproject::Production::create(path).value();
  auto empty = production.readSession().value();
  if (!empty.planRegeneration({}).value().empty())
    throw std::runtime_error("empty planning request");
  auto edit = empty.edit().value();
  const auto asset = edit.importMedia(media).value();
  edit.addExternalIdentifier({postproject::ObjectKind::asset, asset},
      {"https://example.com/id", "camera", std::nullopt}).value();
  edit.addMetadataValue({postproject::ObjectKind::asset, asset},
      "https://example.com/editorial", "title",
      postproject::MetadataValue::plainString("Camera")).value();
  const auto receipt = edit.commitWithReceipt().value();
  if (!receipt.revision || receipt.revision->sequence != 1 ||
      !empty.assets(10).value().items.empty())
    throw std::runtime_error("coherent empty view or receipt");
  if (empty.latestRevision().value() || !empty.changesSince(0, 10).value().empty())
    throw std::runtime_error("coherent empty journal");
  auto view = production.readSession().value();
  const auto latest = view.latestRevision().value();
  const auto filtered = view.changesSinceFiltered(0, {postproject::RevisionEventKind::asset_imported}, 10).value();
  if (filtered.revisions.size() != 1 || filtered.through_sequence != receipt.revision->sequence)
    throw std::runtime_error("coherent filtered journal");
  if (!latest || latest->id != receipt.revision->id || view.changesSince(0, 10).value().size() != 1)
    throw std::runtime_error("coherent journal head");
  const auto events = view.revisionEvents(latest->id, 1).value();
  if (events.items.size() != 1 || !events.next_cursor)
    throw std::runtime_error("bounded revision events");
  const auto next_events = view.revisionEvents(latest->id, 1000, events.next_cursor).value();
  const auto live_events = production.revisionEvents(latest->id, 1000).value();
  if (next_events.items.empty() || next_events.next_cursor ||
      next_events.items[0].position <= events.items[0].position ||
      live_events.items.size() != next_events.items.size() + 1)
    throw std::runtime_error("ordered revision event continuation");
  if (!view.unresolvedMedia(10).value().items.empty() ||
      view.objectsChangedSince(0, 10).value().items.empty() ||
      view.representationsUnderMediaRoot("missing", 10).error().code() != postproject::ErrorCode::not_found)
    throw std::runtime_error("coherent object filters");
  const auto copied = view.asset(asset).value();
  const auto representations = view.representations(asset, 10).value();
  if (representations.items.size() != 1)
    throw std::runtime_error("representation page");
  const auto representation = view.representation(representations.items[0].id).value();
  if (view.evaluateArtifact(representation.id).value().state !=
          postproject::ArtifactKnowledgeState::indeterminate ||
      view.artifactReproducibility(representation.id).value().reproducible)
    throw std::runtime_error("coherent artifact reports");
  if (view.dependencySet(representation.id).value().has_value() ||
      !view.dependencies(representation.id, 64, 1000, 10).value().items.empty() ||
      !view.dependents({postproject::ObjectKind::asset, asset}, 64, 1000, 10).value().items.empty())
    throw std::runtime_error("coherent dependency queries");
  if (!view.ancestors(representation.id, 64, 1000, 10).value().items.empty() ||
      !view.descendants(representation.id, 64, 1000, 10).value().items.empty() ||
      !view.staleArtifacts(64, 1000, 10).value().items.empty())
    throw std::runtime_error("coherent provenance and staleness");
  if (!view.outputsByActivityKind("example:render", 10).value().items.empty() ||
      !view.outputsByTool({"Example", std::nullopt, std::nullopt}, 10).value().items.empty())
    throw std::runtime_error("coherent activity output filters");
  if (!view.activitiesProducing(representation.id, 10).value().items.empty() ||
      !view.activitiesConsuming(representation.id, 10).value().items.empty())
    throw std::runtime_error("coherent activity pages");
  const auto resources = view.resources(representation.id, 10).value();
  if (resources.items.size() != 1)
    throw std::runtime_error("resource page");
  const auto options = postproject::ResolutionOptions::create().value();
  if (view.verifyResource(resources.items[0], media).value() !=
          postproject::ContentVerification::matches ||
      view.resolveAsset(asset).value().size() != 1 ||
      view.resolveAsset(asset, options).value().size() != 1 ||
      view.resolveAssets({asset}).value().size() != 1 ||
      view.resolveAssets({asset}, options).value().size() != 1)
    throw std::runtime_error("coherent resolution and verification");
  const auto locators = view.locators(resources.items[0], 10).value();
  const auto owners = view.representationsUsingResource(resources.items[0], 10).value();
  if (locators.items.size() != 1 || owners.items.size() != 1 ||
      owners.items[0].id != representation.id)
    throw std::runtime_error("locator and resource ownership pages");
  const auto base = view.decisionBase().value();
  const auto token = base.toToken().value();
  const auto parsed = postproject::DecisionBase::fromToken(token).value();
  if (parsed.production_id != base.production_id ||
      parsed.revision->id != base.revision->id ||
      postproject::DecisionBase::fromToken(token + ":extra").has_value())
    throw std::runtime_error("scoped decision token");
  const postproject::ObjectRef target{postproject::ObjectKind::asset, asset};
  const auto title = view.queryMetadata("https://example.com/editorial", "title", 10).value();
  if (title.items.size() != 1 ||
      title.items[0].value.getIf<postproject::MetadataString>()->value != "Camera" ||
      view.queryMetadata("https://example.com/editorial", "title",
          postproject::MetadataValue::plainString("Other"), 10).value().items.size() != 0)
    throw std::runtime_error("coherent metadata queries");
  if (!view.mediaRoots().value().empty() ||
      view.externalIdentifiers(target).value()[0].value != "camera" ||
      view.findByExternalIdentifier("https://example.com/id", "camera").value().size() != 1 ||
      view.findKnownMediaByLocator({postproject::fileLocator(media).value(), std::nullopt}, 10).value().items[0].asset_id != asset ||
      view.findKnownMediaByFingerprint(postproject::fingerprintFile(media).value(), 10).value().items[0].asset_id != asset)
    throw std::runtime_error("coherent lookup projections");
  auto later = production.edit(base).value();
  const auto noop = later.commitWithReceipt().value();
  if (noop.revision || copied.id != asset || representation.asset_id != asset)
    throw std::runtime_error("no-change receipt or copied identity");
  auto before_job = production.readSession().value();
  auto request = before_job.edit().value();
  const auto job = request.requestJob({"example:proxy", {representation.id}, asset,
      postproject::RepresentationKind::proxy, std::nullopt}).value();
  request.commitWithReceipt().value();
  if (!before_job.jobs(10).value().items.empty() ||
      before_job.job(job).error().code() != postproject::ErrorCode::not_found)
    throw std::runtime_error("coherent job view");
  auto after_job = production.readSession().value();
  if (after_job.job(job).value().id != job ||
      after_job.jobs(10, std::nullopt, postproject::JobState::requested,
          "example:proxy").value().items.size() != 1)
    throw std::runtime_error("coherent job point and filtered page");
}
// [/coherent-reads]

int main(int argc, char **argv) {
  if (argc != 2) return 2;
  try { exercise(std::string(argv[1]) + "/views.pproj", std::string(argv[1]) + "/rushes/A001.mov"); }
  catch (const std::exception &error) { std::cerr << error.what() << '\n'; return 1; }
}
