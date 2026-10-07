// Runs every C++ listing included in the PostProject integrator guides.
//
// Each "[name]" ... "[/name]" region is included verbatim by the documentation
// build, so keep regions self-contained and readable. Usage:
//   postproject-cpp-guides WORK_DIRECTORY
// The work directory is prepared by prepare-workdir.cmake.
#include <postproject/postproject.hpp>

#include <algorithm>
#include <chrono>
#include <condition_variable>
#include <cstdint>
#include <cstdio>
#include <iostream>
#include <mutex>
#include <optional>
#include <stdexcept>
#include <string>
#include <string_view>
#include <utility>
#include <variant>
#include <vector>

namespace {

void require(bool condition, const char *message) {
  if (!condition) {
    throw std::runtime_error(message);
  }
}

// [create-production]
std::pair<postproject::Production, postproject::AssetId>
create_production(const std::string &path, const std::string &media) {
  auto production = postproject::Production::create(path, "Documentary").value();

  auto transaction = production.beginTransaction().value();
  transaction.setRevisionContext(
      {postproject::OriginIdentity{"com.example.editor", "0.4.0", std::nullopt},
       "Import camera original"}).value();
  const auto asset_id = transaction.importMedia(media, "Camera A").value();
  transaction.commit().value();

  std::cout << "representations: "
            << production.representations(asset_id).value().size() << '\n';
  return {std::move(production), asset_id};
}
// [/create-production]

// [external-identifiers]
void tag_camera_serial(postproject::Production &production,
                       const postproject::AssetId &asset_id) {
  const postproject::ObjectRef target = postproject::ObjectRef::asset(asset_id);
  const postproject::ExternalIdentifier identifier{
      "com.example.camera.serial", "A-0007", std::nullopt};

  auto transaction = production.beginTransaction().value();
  transaction.addExternalIdentifier(target, identifier).value();
  transaction.commit().value();

  const auto attached = production.externalIdentifiers(target).value();
  const auto matches =
      production.findByExternalIdentifier(identifier.scheme, identifier.value).value();
  require(attached.size() == 1 && matches == std::vector{target},
          "identifier lookup");
}
// [/external-identifiers]

// [metadata]
void add_title(postproject::Production &production,
               const postproject::AssetId &asset_id) {
  const postproject::ObjectRef target = postproject::ObjectRef::asset(asset_id);

  auto transaction = production.beginTransaction().value();
  transaction.addMetadataValue(
      target,
      "https://iptc.org/std/videometadatahub/recommendation/"
      "iptc-vmhub-1.7-schema.json",
      "title", postproject::MetadataValue::languageString("Interview", "en-US")).value();
  transaction.commit().value();
  // Read one target's assertions with the C function pp_production_metadata().
}
// [/metadata]

// [media-root]
void add_rushes_root(postproject::Production &production) {
  auto transaction = production.beginTransaction().value();
  transaction.addMediaRoot("rushes", "Camera originals").value();
  transaction.commit().value();
}
// [/media-root]

// [resolve-asset]
std::vector<postproject::RepresentationResolution>
resolve_asset(const postproject::Production &production,
              const postproject::AssetId &asset_id,
              const std::string &rushes_directory) {
  // The mapping locates the logical root on this machine for this call only.
  auto options = postproject::ResolutionOptions::create().value();
  options.addRootMapping("rushes", rushes_directory).value();
  const auto resolutions = production.resolveAsset(asset_id, options).value();
  for (const auto &representation : resolutions) {
    std::cout << "availability: "
              << static_cast<std::uint32_t>(representation.availability)
              << '\n';
    for (const auto &resource : representation.resources) {
      for (const auto &candidate : resource.candidates) {
        std::cout << "candidate: " << candidate.uri << " ("
                  << candidate.confidence_basis_points << "/10000)\n";
      }
    }
  }
  return resolutions;
}
// [/resolve-asset]

// [confirm-locator]
void confirm_unique_candidates(
    postproject::Production &production,
    const std::vector<postproject::RepresentationResolution> &resolutions) {
  auto transaction = production.beginTransaction().value();
  for (const auto &representation : resolutions) {
    for (const auto &resource : representation.resources) {
      // Several candidates need a person to choose; never pick one here.
      if (resource.candidates.size() != 1) {
        continue;
      }
      const auto &candidate = resource.candidates.front();
      // Record the logical root the candidate was found under and, for an
      // image sequence, the naming of its files there.
      transaction
          .confirmLocator(resource.resource_id, candidate.uri,
                          candidate.media_root, candidate.sequence_naming)
          .value();
    }
  }
  transaction.commit().value();
}
// [/confirm-locator]

// [image-sequence]
postproject::RepresentationId add_render_sequence(postproject::Production &production,
                                      const postproject::AssetId &asset_id,
                                      const std::string &directory) {
  postproject::ImageSequenceInput sequence{};
  sequence.directory = directory;
  sequence.naming = {"shot010.", ".exr", 4};
  sequence.start = 1001;
  sequence.end = 1004;
  sequence.step = 1;
  sequence.rate_numerator = 24000;
  sequence.rate_denominator = 1001;
  sequence.missing_frames = {1003};

  auto transaction = production.beginTransaction().value();
  const auto sequence_id = transaction.addRepresentation(
      asset_id, postproject::RepresentationKind::derived,
      postproject::MediaSource::imageSequence(sequence)).value();
  transaction.commit().value();

  for (const auto &representation : production.representations(asset_id).value()) {
    if (representation.id == sequence_id && representation.imageSequence()) {
      const auto &stored = *representation.imageSequence();
      std::cout << "frames " << stored.start << '-' << stored.end << " at "
                << stored.rate_numerator << '/' << stored.rate_denominator
                << ", "
                << stored.missing_frames.size() << " known missing\n";
    }
  }
  return sequence_id;
}
// [/image-sequence]

// [provenance]
void record_render(postproject::Production &production,
                   const postproject::RepresentationId &source_id,
                   const postproject::RepresentationId &render_id) {
  postproject::ActivitySpec activity{};
  activity.kind = "org.postproject:render";
  activity.inputs = {{source_id, "org.postproject:primary"}};
  activity.outputs = {{render_id, std::nullopt}};
  activity.tool = postproject::ToolIdentity{
      "Example Renderer", "2.1", "https://example.com/renderer"};

  auto transaction = production.beginTransaction().value();
  const auto activity_id = transaction.createActivity(activity).value();
  transaction.commit().value();

  const auto producers = production.activitiesProducing(render_id).value();
  require(producers.size() == 1 && producers.front().id == activity_id,
          "producing activity");
  require(production.ancestors(render_id).value() == std::vector{source_id},
          "provenance ancestors");
  require(production.descendants(source_id).value() == std::vector{render_id},
          "provenance descendants");
}
// [/provenance]

// [artifact-knowledge]
void inspect_artifact(const postproject::Production &production,
                      const postproject::RepresentationId &artifact_id) {
  const auto evaluation = production.evaluateArtifact(artifact_id, 64, 1000).value();
  std::cout << "artifact state: " << static_cast<std::uint32_t>(evaluation.state)
            << '\n';
  for (const auto &reason : evaluation.reasons) {
    std::cout << "reason: " << static_cast<std::uint32_t>(reason.kind) << '\n';
  }

  const auto reproducibility = production.artifactReproducibility(artifact_id).value();
  std::cout << "reproducible: " << reproducibility.reproducible
            << ", missing conditions: " << reproducibility.issues.size()
            << '\n';
}
// [/artifact-knowledge]

// [dependency-queries]
void record_and_query_dependencies(postproject::Production &production,
                                   const postproject::RepresentationId &source_id,
                                   const postproject::AssetId &target_asset_id,
                                   const postproject::RepresentationId &resolved_id) {
  const postproject::ObjectRef target = postproject::ObjectRef::asset(target_asset_id);
  const postproject::Dependency dependency{
      std::nullopt, "org.example:character-reference", target, resolved_id,
      true, "characters/lead.usd"};
  auto transaction = production.readSession().value().edit().value();
  transaction.recordDependencySet(source_id, {dependency}).value();
  transaction.commit().value();

  const auto dependencies = production.dependencies(source_id, 4, 1000, 100).value();
  for (const auto &match : dependencies.items) {
    std::cout << "dependency at depth " << match.depth << '\n';
  }
  require(!dependencies.traversal_truncated, "complete dependency traversal");

  const auto dependents = production.dependents(target, 4, 1000, 100).value();
  require(dependents.items.size() == 1 &&
              dependents.items.front().target == postproject::ObjectRef::representation(source_id),
          "reverse dependency query");
}
// [/dependency-queries]

// [job-query-pages]
void request_and_page_jobs(postproject::Production &production,
                           const postproject::RepresentationId &input_id,
                           const postproject::AssetId &output_asset_id) {
  const postproject::JobRequest request{
      "org.example:generate-proxy", {input_id}, output_asset_id,
      postproject::RepresentationKind::proxy, std::nullopt};
  auto transaction = production.beginTransaction().value();
  const auto job_id = transaction.requestJob(request).value();
  transaction.requestJob(request).value();
  transaction.commit().value();
  require(production.job(job_id).value().stateKind() == postproject::JobState::requested,
          "requested job read back by identity");

  std::optional<std::string> cursor;
  std::size_t count = 0;
  do {
    const auto page = production.jobs(
        1, cursor, postproject::JobState::requested,
        std::string_view("org.example:generate-proxy")).value();
    count += page.items.size();
    cursor = page.next_cursor;
  } while (cursor.has_value());
  require(count == 2, "two requested proxy jobs");
}
// [/job-query-pages]

// [media-structure-pages]
void print_recorded_locators(const postproject::Production &production) {
  std::optional<std::string> cursor;
  do {
    const auto assets = production.assets(100, cursor).value();
    for (const auto &asset : assets.items) {
      // Follow each nested next_cursor the same way in large productions.
      for (const auto &representation :
           production.representations(asset.id, 100).value().items) {
        for (const auto &resource_id :
             production.resources(representation.id, 100).value().items) {
          const auto locators = production.locators(resource_id, 100).value();
          for (const auto &match : locators.items) {
            std::cout << match.locator.uri
                      << " (root: " << match.media_root.value_or("-") << ")\n";
          }
        }
      }
    }
    cursor = assets.next_cursor;
  } while (cursor.has_value());
}
// [/media-structure-pages]

// [knowledge-only-media]
std::vector<postproject::RepresentationId>
list_media_knowledge(const postproject::Production &production) {
  // Both queries read recorded knowledge; neither touches the filesystem.
  const auto unresolved = production.unresolvedMedia(100).value();
  std::cout << "representations without a recorded locator: "
            << unresolved.items.size() << '\n';

  std::vector<postproject::RepresentationId> under_rushes;
  for (const auto &representation :
       production.representationsUnderMediaRoot("rushes", 100).value().items) {
    under_rushes.push_back(representation.id);
  }
  return under_rushes;
}
// [/knowledge-only-media]

// [point-reads]
void read_known_objects(const postproject::Production &production,
                        const postproject::AssetId &asset_id,
                        const postproject::RepresentationId &representation_id) {
  // A host reference names one object; read it without scanning the
  // production.
  const auto asset = production.asset(asset_id).value();
  const auto representation = production.representation(representation_id).value();
  std::cout << asset.display_name.value_or("unnamed") << ": "
            << representation.resources.size() << " resource(s)\n";
  const auto users = production.representationsUsingResource(
      representation.resources[0].id, 100).value();
  require(std::any_of(users.items.begin(), users.items.end(),
                      [&](const postproject::Representation &item) {
                        return item.id == representation_id;
                      }),
          "representation uses its resource");
}
// [/point-reads]

// [metadata-query-pages]
std::vector<postproject::ObjectRef>
find_interview_titles(const postproject::Production &production) {
  const auto page = production.queryMetadata(
      "https://iptc.org/std/videometadatahub/recommendation/"
      "iptc-vmhub-1.7-schema.json",
      "title", postproject::MetadataValue::languageString("Interview", "en-US"),
      100).value();
  std::cout << "exact title matches: " << page.items.size() << '\n';
  std::vector<postproject::ObjectRef> targets;
  for (const auto &assertion : page.items) {
    targets.push_back(assertion.target);
  }
  return targets;
}
// [/metadata-query-pages]

// [provenance-query-pages]
void query_render_lineage(const postproject::Production &production,
                          const postproject::RepresentationId &source_id,
                          const postproject::RepresentationId &render_id) {
  const auto producing = production.activitiesProducing(render_id, 100).value();
  const auto consuming = production.activitiesConsuming(source_id, 100).value();
  require(producing.items.size() == 1 && consuming.items.size() == 1 &&
              producing.items.front().id == consuming.items.front().id,
          "render activity");

  const auto by_kind =
      production.outputsByActivityKind("org.postproject:render", 100).value();
  const auto by_tool = production.outputsByTool(
      {"Example Renderer", "2.1", "https://example.com/renderer"}, 100).value();
  require(by_kind.items == std::vector{render_id} &&
              by_tool.items == std::vector{render_id},
          "render outputs");

  const auto ancestors = production.ancestors(render_id, 8, 1000, 100).value();
  for (const auto &match : ancestors.items) {
    std::cout << "ancestor at depth " << match.depth << '\n';
  }
  require(!ancestors.traversal_truncated, "complete ancestor traversal");

  const auto descendants = production.descendants(source_id, 8, 1000, 100).value();
  require(descendants.items.front().object == postproject::ObjectRef::representation(render_id),
          "render descends from its source");
}
// [/provenance-query-pages]

// [stale-artifact-pages]
std::vector<postproject::RepresentationId>
stale_descendants(const postproject::Production &production,
                  const postproject::RepresentationId &source_id) {
  std::vector<postproject::RepresentationId> stale;
  std::optional<std::string> cursor;
  do {
    const auto page =
        production.staleArtifacts(64, 1000, 100, cursor, source_id).value();
    // A page bounds the candidates examined, so it may hold fewer stale
    // results, or none, and still carry a continuation.
    stale.insert(stale.end(), page.items.begin(), page.items.end());
    cursor = page.next_cursor;
  } while (cursor.has_value());
  return stale;
}
// [/stale-artifact-pages]

// [changed-objects]
std::vector<postproject::ObjectRef>
objects_changed_after(const postproject::Production &production,
                      std::uint64_t sequence) {
  std::vector<postproject::ObjectRef> changed;
  std::optional<std::string> cursor;
  do {
    const auto page = production.objectsChangedSince(sequence, 100, cursor).value();
    changed.insert(changed.end(), page.items.begin(), page.items.end());
    cursor = page.next_cursor;
  } while (cursor.has_value());
  return changed;
}
// [/changed-objects]

void handle_event(const postproject::RevisionEvent &event) {
  std::cout << "event " << event.position << ": alternative "
            << event.payload.index() << '\n';
}

// [revision-feed]
std::uint64_t process_changes(const postproject::Production &production,
                              std::uint64_t cursor) {
  constexpr std::uint32_t limit = 100;
  for (;;) {
    const auto page = production.changesSince(cursor, limit).value();
    for (const auto &revision : page) {
      for (const auto &event : production.revisionEvents(revision.id).value()) {
        // Dispatch with std::visit on event.payload.
        handle_event(event);
      }
      // Persist the cursor only after the whole revision is processed.
      cursor = revision.sequence;
    }
    if (page.size() < limit) {
      return cursor;
    }
  }
}
// [/revision-feed]

// [revision-filter]
std::pair<std::vector<postproject::RevisionId>, std::uint64_t>
new_media_revisions(const postproject::Production &production,
                    std::uint64_t cursor) {
  const auto page = production.changesSinceFiltered(
      cursor, {postproject::RevisionEventKind::representation_added,
               postproject::RevisionEventKind::job_succeeded}).value();
  std::vector<postproject::RevisionId> revisions;
  for (const auto &revision : page.revisions) {
    revisions.push_back(revision.id);
  }
  // Continue from the through sequence, which skips unrelated revisions.
  return {revisions, page.through_sequence};
}
// [/revision-filter]

// [revision-wait]
std::vector<postproject::Revision>
wait_for_changes(const postproject::Production &production,
                 std::uint64_t cursor) {
  auto waiter = production.revisionWaiter().value();
  // Another thread may call waiter.cancel() to stop the wait.
  auto wait = waiter.wait(cursor, 100, std::chrono::seconds(5)).value();
  return std::move(wait.revisions); // empty unless result is revisions
}

// The callback runs on the observer's own thread; destroy the observer
// before the production.
std::unique_ptr<postproject::RevisionObserver>
watch_new_media(const postproject::Production &production,
                std::uint64_t cursor,
                postproject::RevisionObserver::Callback on_revision) {
  return std::make_unique<postproject::RevisionObserver>(
      production, cursor, std::move(on_revision),
      std::vector{postproject::RevisionEventKind::representation_added});
}
// [/revision-wait]

// [host-binding]
std::string bind_representation(const postproject::Production &production,
                                const postproject::RepresentationId &representation_id) {
  const postproject::HostObjectBinding binding{
      production.id().value(), postproject::ObjectRef::representation(representation_id)};
  const std::string stored = binding.toString().value();

  const auto reopened = postproject::HostObjectBinding::fromString(stored).value();
  require(reopened == binding, "binding round trip");
  return stored;
}
// [/host-binding]

} // namespace

int main(int argc, char **argv) {
  if (argc != 2) {
    std::cerr << "usage: postproject-cpp-guides WORK_DIRECTORY\n";
    return 2;
  }
  const std::string work = argv[1];
  const std::string media = work + "/rushes/A001.mov";
  const std::string moved = work + "/moved";

  try {
    auto [production, asset_id] =
        create_production(work + "/production.pproj", media);
    const auto original_id = production.representations(asset_id).value().front().id;
    tag_camera_serial(production, asset_id);
    add_title(production, asset_id);

    add_rushes_root(production);
    require(std::rename(media.c_str(), (moved + "/A001.mov").c_str()) == 0,
            "move media");
    const auto resolutions = resolve_asset(production, asset_id, moved);
    require(resolutions.front().resources.front().candidates.size() == 1,
            "unique candidate");
    confirm_unique_candidates(production, resolutions);

    const auto before_render = production.latestRevision().value()->sequence;
    const auto sequence_id = add_render_sequence(
        production, asset_id, work + "/renders/shot010");
    record_render(production, original_id, sequence_id);
    inspect_artifact(production, sequence_id);
    record_and_query_dependencies(production, sequence_id, asset_id,
                                  original_id);
    request_and_page_jobs(production, original_id, asset_id);

    print_recorded_locators(production);
    require(list_media_knowledge(production) == std::vector{original_id},
            "representation under the rushes root");
    read_known_objects(production, asset_id, original_id);
    const postproject::ObjectRef asset = postproject::ObjectRef::asset(asset_id);
    require(find_interview_titles(production) == std::vector{asset},
            "exact title match");
    query_render_lineage(production, original_id, sequence_id);
    require(stale_descendants(production, original_id).empty(),
            "no stale renders");
    const auto sequence = postproject::ObjectRef::representation(sequence_id);
    const auto changed = objects_changed_after(production, before_render);
    require(std::find(changed.begin(), changed.end(), sequence) !=
                changed.end(),
            "changed render sequence");

    const auto cursor = process_changes(production, 0);
    require(cursor == production.latestRevision().value()->sequence, "feed cursor");
    const auto [media_revisions, through] = new_media_revisions(production, 0);
    require(!media_revisions.empty() && through == cursor, "filtered feed");
    require(!wait_for_changes(production, 0).empty(), "revision wait");
    {
      std::mutex mutex;
      std::condition_variable observed;
      bool delivered = false;
      auto observer = watch_new_media(
          production, 0,
          [&](const postproject::Revision &,
              const std::vector<postproject::RevisionEvent> &) {
            const std::lock_guard<std::mutex> lock(mutex);
            delivered = true;
            observed.notify_all();
          });
      std::unique_lock<std::mutex> lock(mutex);
      require(observed.wait_for(lock, std::chrono::seconds(60),
                                [&] { return delivered; }),
              "observed revision");
      lock.unlock();
      observer->stop();
      require(!observer->error().has_value(), "observer error");
    }
    std::cout << "binding: " << bind_representation(production, sequence_id)
              << '\n';
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
