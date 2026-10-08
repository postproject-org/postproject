#include <postproject/postproject.hpp>

#include <algorithm>
#include <chrono>
#include <condition_variable>
#include <cstdint>
#include <cstdio>
#include <exception>
#include <filesystem>
#include <fstream>
#include <mutex>
#include <optional>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

template <class T> struct EventTag {
  using type = T;
};

namespace fs = std::filesystem;

void write_frame(const fs::path &directory, const std::string &prefix,
                 int frame, const std::string &content) {
  char name[64];
  std::snprintf(name, sizeof(name), "%04d.png", frame);
  std::ofstream file(directory / (prefix + name), std::ios::binary);
  file << content;
}

bool has_evidence(const postproject::ResolutionCandidate &candidate,
                  postproject::EvidenceKind kind) {
  return std::any_of(candidate.evidence.begin(), candidate.evidence.end(),
                     [&](const postproject::Evidence &evidence) {
                       return evidence.kind == kind;
                     });
}

// A sequence imported as shot_0001.png to shot_0003.png and renamed into
// another directory as shot-graded_0001.png ... is found by content and
// confirmed under its new naming beside the original one. A group with the
// same names but other content and an incomplete group are no candidates; two
// identical renamed copies are ambiguous.
int renamed_sequence_scenario(const std::string &production_path) {
  const fs::path base = fs::path(production_path + ".renamed-sequence");
  fs::remove_all(base);
  std::remove((production_path + ".renamed").c_str());
  const fs::path plates = base / "plates";
  const fs::path graded = base / "graded";
  const fs::path other = base / "other";
  const fs::path partial = base / "partial";
  const fs::path copy = base / "copy";
  for (const fs::path &directory : {plates, graded, other, partial, copy}) {
    fs::create_directories(directory);
  }
  const std::string contents[] = {"frame one", "frame two", "frame three"};
  for (int frame = 1; frame <= 3; ++frame) {
    write_frame(plates, "shot_", frame, contents[frame - 1]);
    write_frame(other, "shot-graded_", frame, "other content");
  }
  write_frame(partial, "shot-graded_", 1, contents[0]);
  write_frame(partial, "shot-graded_", 2, contents[1]);

  auto production = postproject::Production::create(
                        production_path + ".renamed", "Renamed sequence")
                        .value();
  const postproject::SequenceNaming original{"shot_", ".png", 4};
  const auto source = postproject::MediaSource::imageSequence(
      {plates.string(), original, 1, 3, 1, 24, 1, {}});
  auto import = production.beginTransaction().value();
  const auto asset_id = import.importMedia(source, "Shot").value();
  import.commit().value();
  for (int frame = 1; frame <= 3; ++frame) {
    char number[16];
    std::snprintf(number, sizeof(number), "%04d.png", frame);
    fs::rename(plates / (std::string("shot_") + number),
               graded / (std::string("shot-graded_") + number));
  }

  auto options = postproject::ResolutionOptions::create().value();
  options.addSearchDirectory(base.string()).value();
  const auto resolved = production.resolveAsset(asset_id, options).value();
  const auto &resource = resolved[0].resources[0];
  const postproject::SequenceNaming renamed{"shot-graded_", ".png", 4};
  if (resource.state() != postproject::ResourceResolutionState::resolved_probable ||
      resource.candidates().size() != 1 ||
      resource.candidates()[0].sequence_naming != renamed ||
      resource.candidates()[0].uri.find("graded") == std::string::npos ||
      !has_evidence(resource.candidates()[0],
                    postproject::EvidenceKind::partial_fingerprint_match)) {
    return 70;
  }
  const auto &candidate = resource.candidates()[0];
  auto confirmation = production.readSession().value().edit().value();
  confirmation
      .confirmLocator(resource.resource_id, candidate.uri, candidate.media_root,
                      candidate.sequence_naming)
      .value();
  confirmation.commit().value();

  const auto locators = production.locators(resource.resource_id, 10).value();
  std::vector<postproject::SequenceNaming> namings;
  for (const auto &item : locators.items) {
    if (item.locator.sequence_naming.has_value()) {
      namings.push_back(*item.locator.sequence_naming);
    }
  }
  if (namings.size() != 2 ||
      std::find(namings.begin(), namings.end(), original) == namings.end() ||
      std::find(namings.begin(), namings.end(), renamed) == namings.end()) {
    return 71;
  }

  // An identical renamed copy makes the search ambiguous once the confirmed
  // directory no longer holds the frames.
  for (int frame = 1; frame <= 3; ++frame) {
    write_frame(copy, "shot-graded_", frame, contents[frame - 1]);
  }
  fs::remove_all(graded);
  fs::create_directories(graded);
  for (int frame = 1; frame <= 3; ++frame) {
    write_frame(plates, "shot-graded_", frame, contents[frame - 1]);
  }
  const auto ambiguous = production.resolveAsset(asset_id, options).value();
  if (ambiguous[0].resources[0].state() !=
          postproject::ResourceResolutionState::ambiguous ||
      ambiguous[0].resources[0].candidates().size() != 2) {
    return 72;
  }
  return 0;
}

int main(int argc, char **argv) {
  if (argc != 2) {
    return 2;
  }

  const std::string path(argv[1]);
  std::remove(path.c_str());

  try {
    if (postproject::abi_version() != 51) {
      return 3;
    }

    auto production = postproject::Production::create(path, "C++ smoke test").value();
    const auto created_id = production.id().value();
    const std::string media_path = path + ".media";
    {
      std::ofstream media(media_path, std::ios::binary);
      media << "C++ transaction media";
      if (!media) {
        return 10;
      }
    }
    auto transaction = production.beginTransaction().value();
    transaction.setRevisionContext(
        {postproject::OriginIdentity{"C++ smoke", std::string("1.0"),
                                     std::nullopt},
         std::string("Import fixture")}).value();
    const auto asset_id = transaction.importMedia(media_path, "C++ asset").value();
    const postproject::ObjectRef asset_ref = postproject::ObjectRef::asset(asset_id);
    const postproject::HostObjectBinding host_binding{created_id, asset_ref};
    const auto host_binding_text = host_binding.toString().value();
    if (host_binding_text.rfind("https://postproject.org/ref/v1/", 0) != 0 ||
        !(postproject::HostObjectBinding::fromString(host_binding_text).value() ==
          host_binding)) {
      return 21;
    }
    const postproject::ExternalIdentifier external_id{
        "com.example.asset", "asset-42", std::string("primary")};
    transaction.addExternalIdentifier(asset_ref, external_id).value();
    const auto root_id = transaction.addMediaRoot("fixtures", "Fixture media").value();
    transaction.commit().value();
    const auto latest_revision = production.latestRevision().value();
    const auto revision_page = production.changesSince(0, 1).value();
    if (!latest_revision.has_value() || latest_revision->sequence != 1 ||
        !latest_revision->origin.has_value() ||
        latest_revision->origin->name != "C++ smoke" ||
        latest_revision->origin->version != std::string("1.0") ||
        latest_revision->message != std::string("Import fixture") ||
        revision_page.size() != 1 ||
        revision_page[0].id != latest_revision->id) {
      return 15;
    }
    const auto revision_events = production.revisionEvents(latest_revision->id).value();
    if (revision_events.size() != 7 || revision_events[0].position != 0 ||
        !std::holds_alternative<postproject::AssetImportedEvent>(
            revision_events[0].payload) ||
        std::get<postproject::AssetImportedEvent>(revision_events[0].payload)
                .asset_id != asset_id ||
        !std::holds_alternative<postproject::ExternalIdentifierAddedEvent>(
            revision_events[5].payload) ||
        std::get<postproject::ExternalIdentifierAddedEvent>(
            revision_events[5].payload)
                .identifier.value != external_id.value ||
        !std::holds_alternative<postproject::MediaRootAddedEvent>(
            revision_events[6].payload)) {
      return 16;
    }
    if (!production.containsAsset(asset_id).value()) {
      return 8;
    }
    const auto assets = production.assets().value();
    if (assets.size() != 1 || assets[0].id != asset_id ||
        assets[0].created_at_unix_micros == 0 ||
        assets[0].display_name != std::string("C++ asset") ||
        assets[0].import_source.has_value()) {
      return 24;
    }
    const auto asset_page = production.assets(1).value();
    if (asset_page.items.size() != 1 || asset_page.items[0].id != asset_id ||
        asset_page.items[0].display_name != std::string("C++ asset") ||
        asset_page.next_cursor.has_value() || asset_page.traversal_truncated) {
      return 40;
    }
    const auto roots = production.mediaRoots().value();
    if (roots.size() != 1 || roots[0].id != root_id ||
        roots[0].name != "fixtures" ||
        roots[0].label != std::string("Fixture media") ||
        roots[0].legacy_uri.has_value() || roots[0].priority != 0 ||
        !roots[0].enabled) {
      return 25;
    }
    const auto representations = production.representations(asset_id).value();
    if (representations.size() != 1 ||
        representations[0].asset_id != asset_id ||
        representations[0].kind != postproject::RepresentationKind::original ||
        representations[0].structureKind() !=
            postproject::ContentStructureKind::single_resource ||
        representations[0].members().size() != 1 ||
        !representations[0].members()[0].required ||
        representations[0].members()[0].role.has_value() ||
        representations[0].imageSequence() != nullptr ||
        representations[0].fingerprints.size() != 1 ||
        representations[0].fingerprints[0].version != 2 ||
        representations[0].fingerprints[0].value.empty() ||
        representations[0].resources.size() != 1 ||
        representations[0].resources[0].id !=
            representations[0].members()[0].resource_id ||
        representations[0].resources[0].file_size != UINT64_C(21) ||
        !representations[0].resources[0].modified_at_unix_micros.has_value() ||
        representations[0].resources[0].fingerprints.size() != 1 ||
        representations[0].resources[0].fingerprints[0].version != 1 ||
        representations[0].resources[0].fingerprints[0].value.empty() ||
        representations[0].resources[0].locators.size() != 1 ||
        representations[0].resources[0].locators[0].availability !=
            postproject::LocatorAvailability::online ||
        !representations[0]
             .resources[0]
             .locators[0]
             .last_seen_unix_micros.has_value()) {
      return 17;
    }
    const auto conflict_resource_id = representations[0].resources[0].id;
    auto first_writer = production.beginTransaction(latest_revision->id).value();
    first_writer
        .confirmLocator(conflict_resource_id,
                        "file:///cpp-smoke/conflict-first.mov")
        .value();
    first_writer.commit().value();
    const auto superseding_revision = production.latestRevision().value();
    auto stale_writer = production.beginTransaction(latest_revision->id).value();
    stale_writer
        .confirmLocator(conflict_resource_id,
                        "file:///cpp-smoke/conflict-stale.mov")
        .value();
    const auto stale_commit = stale_writer.commit();
    if (stale_commit.has_value() || !superseding_revision.has_value()) {
      return 140;
    }
    const auto *conflict = stale_commit.error().transactionConflict();
    if (stale_commit.error().code() != postproject::ErrorCode::conflict ||
        conflict == nullptr ||
        conflict->key.kind != postproject::ConflictKeyKind::locator_set ||
        std::get<postproject::ObjectRef>(conflict->key.target).resourceId().value() != conflict_resource_id ||
        std::get<postproject::ObjectRef>(conflict->key.target).kind() != postproject::ObjectKind::resource ||
        conflict->base_revision_id != latest_revision->id ||
        conflict->base_revision_sequence != latest_revision->sequence ||
        conflict->superseding_revision_id != superseding_revision->id ||
        conflict->superseding_revision_sequence !=
            superseding_revision->sequence) {
      return 141;
    }
    const auto by_locator = production
                                .findKnownMediaByLocator(
                                    {representations[0]
                                         .resources[0]
                                         .locators[0]
                                         .uri,
                                     std::nullopt},
                                    1)
                                .value();
    const auto by_fingerprint =
        production
            .findKnownMediaByFingerprint(
                representations[0].resources[0].fingerprints[0], 1)
            .value();
    for (const auto *matches : {&by_locator, &by_fingerprint}) {
      if (matches->items.size() != 1 || matches->next_cursor.has_value() ||
          matches->traversal_truncated ||
          matches->items[0].asset_id != asset_id ||
          matches->items[0].representation_id != representations[0].id ||
          matches->items[0].resource_id != representations[0].resources[0].id) {
        return 73;
      }
    }
    if (production.dependencySet(representations[0].id).value().has_value() ||
        !production.dependents(asset_ref, 1, 1000, 1000).value().items.empty()) {
      return 30;
    }
    auto observations = production.readSession().value().edit().value();
    observations.recordResourceFingerprint(
        representations[0].resources[0].id,
        {"cpp-smoke", 1, {UINT8_C(0x10), UINT8_C(0x20)}}).value();
    observations.recordRepresentationFingerprint(
        representations[0].id,
        {"cpp-smoke-tree", 1, {UINT8_C(0x30), UINT8_C(0x40)}}).value();
    observations.commit().value();
    const auto observation_revision = production.latestRevision().value();
    if (!observation_revision.has_value()) {
      return 28;
    }
    const auto observation_events =
        production.revisionEvents(observation_revision->id).value();
    if (observation_events.size() != 2 ||
        !std::holds_alternative<postproject::ResourceFingerprintObservedEvent>(
            observation_events[0].payload) ||
        !std::holds_alternative<
            postproject::RepresentationFingerprintObservedEvent>(
            observation_events[1].payload)) {
      return 29;
    }
    const auto observed_representations = production.representations(asset_id).value();
    if (observed_representations[0].fingerprints.size() != 2 ||
        observed_representations[0].resources[0].fingerprints.size() != 2) {
      return 26;
    }
    const auto computed = postproject::fingerprintFile(media_path).value();
    const auto &stored = representations[0].resources[0].fingerprints[0];
    if (computed.algorithm != stored.algorithm ||
        computed.version != stored.version || computed.value != stored.value ||
        production.verifyResource(representations[0].resources[0].id,
                                  media_path).value() !=
            postproject::ContentVerification::matches) {
      return 61;
    }
    const auto &locator_uri =
        representations[0].resources[0].locators[0].uri;
    if (postproject::fileLocator(media_path).value() != locator_uri ||
        postproject::fileLocator(postproject::locatorFilePath(locator_uri).value()).value() !=
            locator_uri) {
      return 62;
    }
    auto content_observation = production.readSession().value().edit().value();
    if (content_observation.observeResourceContent(
            representations[0].resources[0].id, media_path).value() !=
        postproject::ContentObservationOutcome::unchanged) {
      return 65;
    }
    content_observation.commit().value();
    const auto identifiers = production.externalIdentifiers(asset_ref).value();
    const auto found =
        production.findByExternalIdentifier("com.example.asset", "asset-42",
                                            external_id.qualifier).value();
    if (identifiers.size() != 1 ||
        identifiers[0].scheme != external_id.scheme ||
        identifiers[0].value != external_id.value ||
        identifiers[0].qualifier != external_id.qualifier || found.size() != 1 ||
        !(found[0] == asset_ref)) {
      return 13;
    }

    auto rolled_back = production.beginTransaction().value();
    const auto discarded_id = rolled_back.importMedia(media_path).value();
    rolled_back.rollback().value();
    if (production.containsAsset(discarded_id).value()) {
      return 9;
    }

    const std::string moved_media_path = media_path + ".moved";
    std::filesystem::rename(media_path, moved_media_path);
    const std::string fixtures =
        std::filesystem::path(path).parent_path().string();
    auto options = postproject::ResolutionOptions::create().value();
    options.addRootMapping("fixtures", fixtures).value();
    options.addSearchDirectory(fixtures).value();
    options.setVerification(postproject::VerificationMode::presence).value();
    options.setLimits(64, 1000000).value();
    {
      auto token = postproject::CancelToken::create().value();
      options.setCancelToken(token).value();
    }
    const auto resolutions = production.resolveAssets({asset_id}, options).value();
    if (resolutions.size() != 1 || !(resolutions[0].asset_id == asset_id) ||
        resolutions[0].resources[0].candidates().empty() ||
        resolutions[0].resources[0].candidates()[0].media_root !=
            std::optional<std::string>("fixtures") ||
        resolutions[0].availability !=
            postproject::RepresentationAvailability::online ||
        resolutions[0].resources.size() != 1 ||
        resolutions[0].resources[0].state() !=
            postproject::ResourceResolutionState::resolved_exact ||
        resolutions[0].resources[0].candidates().size() != 1 ||
        resolutions[0]
                .resources[0]
                .candidates()[0]
                .confidence_basis_points != 10000 ||
        resolutions[0].resources[0].candidates()[0].evidence.empty()) {
      return 11;
    }
    auto cancel_token = postproject::CancelToken::create().value();
    options.setCancelToken(cancel_token).value();
    cancel_token.cancel();
    try {
      (void)production.resolveAsset(asset_id, options).value();
      return 63;
    } catch (const postproject::Exception &error) {
      if (error.code() != postproject::ErrorCode::cancelled) {
        return 64;
      }
    }

    const postproject::ActivitySpec activity_spec{
        "org.postproject:ingest",
        100,
        200,
        postproject::ToolIdentity{
            "C++ ingest", std::string("1.0"),
            std::string("https://example.com/tools/ingest")},
        postproject::AgentIdentity{
            std::string("C++ operator"),
            postproject::ExternalIdentifier{"com.example.agent", "operator-1",
                                            std::string("primary")}},
        {},
        {{resolutions[0].representation_id,
          std::string("org.postproject:output.master")}}};
    const auto pre_provenance_revision = production.latestRevision().value();
    if (!pre_provenance_revision.has_value()) {
      return 41;
    }
    auto provenance = production.beginTransaction().value();
    const auto activity_id = provenance.createActivity(activity_spec).value();
    std::vector<postproject::MetadataValue> metadata_items;
    metadata_items.push_back(postproject::MetadataValue::rational(24000, 1001));
    metadata_items.push_back(
        postproject::MetadataValue::languageString("Interview", "en-US"));
    std::vector<postproject::MetadataField> metadata_fields;
    metadata_fields.push_back(
        {"values", postproject::MetadataValue::list(metadata_items)});
    const auto metadata =
        postproject::MetadataValue::structure(metadata_fields);
    provenance.addMetadataValue(
        postproject::ObjectRef::activity(activity_id),
        "com.example.ingest", "details", metadata).value();
    provenance.addMetadataValue(asset_ref, "com.example.ingest", "title",
                                postproject::MetadataValue::plainString(
                                    "Interview")).value();
    provenance.commit().value();

    const auto title_matches = production.queryMetadata(
        "com.example.ingest", "title",
        postproject::MetadataValue::plainString("Interview"), 10).value();
    const auto title_misses = production.queryMetadata(
        "com.example.ingest", "title",
        postproject::MetadataValue::plainString("Other"), 10).value();
    const auto detail_matches =
        production.queryMetadata("com.example.ingest", "details", 1).value();
    if (title_matches.items.size() != 1 ||
        !(title_matches.items[0].target == asset_ref) ||
        title_matches.items[0].vocabulary != "com.example.ingest" ||
        title_matches.items[0].property != "title" ||
        title_matches.next_cursor.has_value() ||
        !title_misses.items.empty() || title_misses.next_cursor.has_value() ||
        detail_matches.items.size() != 1 ||
        !(detail_matches.items[0].target ==
          postproject::ObjectRef::activity(activity_id)) ||
        detail_matches.next_cursor.has_value()) {
      return 42;
    }
    try {
      static_cast<void>(production.queryMetadata(
          "com.example.ingest", "details", metadata, 10).value());
      return 43;
    } catch (const postproject::Exception &error) {
      if (error.code() != postproject::ErrorCode::invalid_argument) {
        return 43;
      }
    }

    std::vector<postproject::ObjectRef> changed_objects;
    std::optional<std::string> changed_cursor;
    do {
      const auto changed = production.objectsChangedSince(
          pre_provenance_revision->sequence, 1, changed_cursor).value();
      if (changed.items.size() > 1 || changed.traversal_truncated) {
        return 44;
      }
      changed_objects.insert(changed_objects.end(), changed.items.begin(),
                             changed.items.end());
      changed_cursor = changed.next_cursor;
    } while (changed_cursor.has_value());
    const auto contains_object = [&](const postproject::ObjectRef &object) {
      return std::any_of(changed_objects.begin(), changed_objects.end(),
                         [&](const postproject::ObjectRef &candidate) {
                           return candidate == object;
                         });
    };
    if (changed_objects.size() < 2 || !contains_object(asset_ref) ||
        !contains_object(postproject::ObjectRef::activity(activity_id))) {
      return 44;
    }

    const auto activities = production.activities().value();
    const auto producing =
        production.activitiesProducing(resolutions[0].representation_id).value();
    const auto producing_page =
        production.activitiesProducing(resolutions[0].representation_id, 1).value();
    const auto ingest_outputs =
        production.outputsByActivityKind("org.postproject:ingest", 1).value();
    if (producing_page.items.size() != 1 ||
        producing_page.items[0].id != activity_id ||
        producing_page.items[0].kind != "org.postproject:ingest" ||
        producing_page.next_cursor.has_value() ||
        ingest_outputs.items !=
            std::vector<postproject::RepresentationId>{resolutions[0].representation_id} ||
        ingest_outputs.next_cursor.has_value() ||
        ingest_outputs.traversal_truncated ||
        !production.unresolvedMedia(100).value().items.empty()) {
      return 45;
    }
    if (activities.size() != 1 || producing.size() != 1 ||
        activities[0].id != activity_id ||
        activities[0].kind != "org.postproject:ingest" ||
        activities[0].started_at_unix_micros != 100 ||
        activities[0].finished_at_unix_micros != 200 ||
        !activities[0].tool.has_value() ||
        activities[0].tool->name != "C++ ingest" ||
        !activities[0].agent.has_value() ||
        activities[0].agent->name != std::string("C++ operator") ||
        activities[0].outputs.size() != 1 ||
        activities[0].outputs[0].representation_id !=
            resolutions[0].representation_id ||
        activities[0].outputs[0].role !=
            std::string("org.postproject:output.master") ||
        !activities[0].outputs[0].snapshot.has_value() ||
        activities[0].outputs[0].snapshot->revision_sequence == 0 ||
        activities[0].outputs[0].snapshot->fingerprints.empty() ||
        !activities[0]
             .outputs[0]
             .snapshot->fingerprints[0]
             .observed_revision_sequence.has_value() ||
        !production.ancestors(resolutions[0].representation_id).value().empty()) {
      return 14;
    }
    const auto artifact =
        production.evaluateArtifact(resolutions[0].representation_id).value();
    const auto reproducibility =
        production.artifactReproducibility(resolutions[0].representation_id).value();
    if (artifact.state != postproject::ArtifactKnowledgeState::current ||
        artifact.visited_representations != 1 || artifact.truncated ||
        !artifact.reasons.empty() || !reproducibility.reproducible ||
        reproducibility.producing_activity_id != activity_id ||
        reproducibility.activity_kind !=
            std::string("org.postproject:ingest") ||
        !reproducibility.issues.empty()) {
      return 28;
    }
    {
      auto unbased = production.beginTransaction().value();
      const auto enabling = unbased.setMediaRootEnabled(root_id, true);
      const auto removal = unbased.removeMediaRoot(root_id);
      const auto retirement = unbased.retireLocator(representations[0].resources[0].locators[0].id);
      const auto identifier_removal = unbased.removeExternalIdentifier(
          postproject::ObjectRef::asset(asset_id), {"com.example.id", "observed", std::nullopt});
      const auto dependency_replacement = unbased.recordDependencySet(representations[0].id, {});
      if (enabling || removal || retirement || identifier_removal || dependency_replacement ||
          dependency_replacement.error().code() != postproject::ErrorCode::invalid_argument ||
          identifier_removal.error().code() != postproject::ErrorCode::invalid_argument ||
          retirement.error().code() != postproject::ErrorCode::invalid_argument ||
          enabling.error().code() != postproject::ErrorCode::invalid_argument ||
          removal.error().code() != postproject::ErrorCode::invalid_argument)
        return 151;
      unbased.addMediaRoot("rejection-recovery").value();
      unbased.rollback().value();
    }
    auto confirmation = production.readSession().value().edit().value();
    confirmation.confirmLocator(
        resolutions[0].resources[0].resource_id,
        resolutions[0].resources[0].candidates()[0].uri,
        std::string("fixtures")).value();
    confirmation.setMediaRootEnabled(root_id, false).value();
    confirmation.retireLocator(
        representations[0].resources[0].locators[0].id).value();
    confirmation.commit().value();

    const auto rooted = production.representationsUnderMediaRoot("fixtures", 1).value();
    const auto resource_page =
        production.resources(resolutions[0].representation_id, 1).value();
    const auto locator_page =
        production.locators(resolutions[0].resources[0].resource_id, 10).value();
    const auto rooted_locator = std::find_if(
        locator_page.items.begin(), locator_page.items.end(),
        [&](const postproject::ResourceLocator &candidate) {
          return candidate.locator.uri ==
                 resolutions[0].resources[0].candidates()[0].uri;
        });
    if (rooted.items.size() != 1 ||
        rooted.items[0].id != resolutions[0].representation_id ||
        rooted.items[0].resources.size() != 1 ||
        rooted.next_cursor.has_value() ||
        resource_page.items !=
            std::vector<postproject::ResourceId>{
                resolutions[0].resources[0].resource_id} ||
        resource_page.next_cursor.has_value() ||
        locator_page.items.empty() || locator_page.next_cursor.has_value() ||
        rooted_locator == locator_page.items.end() ||
        rooted_locator->resource_id !=
            resolutions[0].resources[0].resource_id ||
        rooted_locator->media_root != std::string("fixtures") ||
        rooted_locator->locator.availability !=
            postproject::LocatorAvailability::online ||
        !rooted_locator->locator.last_seen_unix_micros.has_value()) {
      return 46;
    }
    try {
      static_cast<void>(
          production.locators(resolutions[0].resources[0].resource_id, 10,
                              std::string_view("not-a-cursor")).value());
      return 47;
    } catch (const postproject::Exception &error) {
      if (error.code() != postproject::ErrorCode::invalid_argument) {
        return 47;
      }
    }
    const auto point_asset = production.asset(asset_id).value();
    const auto point_representation =
        production.representation(resolutions[0].representation_id).value();
    const auto resource_users = production.representationsUsingResource(
        resolutions[0].resources[0].resource_id, 10).value();
    if (point_asset.id != asset_id ||
        point_representation.id != resolutions[0].representation_id ||
        point_representation.asset_id != asset_id ||
        resource_users.items.size() != 1 ||
        resource_users.items[0].id != resolutions[0].representation_id ||
        resource_users.next_cursor.has_value()) {
      return 58;
    }
    try {
      static_cast<void>(
          production.representation(postproject::RepresentationId(postproject::Uuid::Bytes{})).value());
      return 59;
    } catch (const postproject::Exception &error) {
      if (error.code() != postproject::ErrorCode::not_found) {
        return 59;
      }
    }
    const auto disabled_roots = production.mediaRoots().value();
    if (disabled_roots.size() != 1 || disabled_roots[0].enabled) {
      return 26;
    }
    auto root_removal = production.readSession().value().edit().value();
    root_removal.removeMediaRoot(root_id).value();
    root_removal.commit().value();
    if (!production.mediaRoots().value().empty()) {
      return 27;
    }

    auto moved = std::move(production);
    if (production || !moved) {
      return 4;
    }

    auto reopened = postproject::Production::open(path).value();
    if (reopened.id().value() != created_id || !reopened.containsAsset(asset_id).value()) {
      return 5;
    }
    const auto persisted = reopened.resolveAsset(asset_id).value();
    if (persisted.size() != 1 ||
        persisted[0].availability !=
            postproject::RepresentationAvailability::online ||
        persisted[0].resources.size() != 1 ||
        persisted[0].resources[0].state() !=
            postproject::ResourceResolutionState::online_at_known_locator) {
      return 12;
    }

    const auto sequence_frame_path =
        std::filesystem::path(path).parent_path() / "frame0001.exr";
    {
      std::ofstream frame(sequence_frame_path, std::ios::binary);
      frame << "sequence frame";
      if (!frame) {
        return 22;
      }
    }
    auto additions = reopened.beginTransaction().value();
    const auto proxy_id = additions.addRepresentation(
        asset_id, postproject::RepresentationKind::proxy, moved_media_path).value();
    const auto sequence = postproject::MediaSource::imageSequence(
        {sequence_frame_path.parent_path().string(),
         {"frame", ".exr", 4},
         1,
         1,
         1,
         24000,
         1001,
         {}});
    const auto sequence_id = additions.addRepresentation(
        asset_id, postproject::RepresentationKind::derived, sequence).value();
    // The same sequence imported as a new asset is that asset's only original.
    const auto strip_id = additions.importMedia(sequence, "C++ image strip").value();
    const auto ordered = postproject::MediaSource::orderedParts({
        {moved_media_path, "org.postproject:essence.first", true},
        {sequence_frame_path.string(), "org.postproject:essence.second", true},
    });
    const auto ordered_id = additions.addRepresentation(
        asset_id, postproject::RepresentationKind::optimized, ordered).value();
    const auto package = postproject::MediaSource::package({
        {moved_media_path, "org.postproject:essence", true},
        {sequence_frame_path.string(), "org.postproject:sidecar", false},
    });
    const auto package_id = additions.addRepresentation(
        asset_id, postproject::RepresentationKind::derived, package).value();
    const auto optional_part = additions.addRepresentation(
        asset_id, postproject::RepresentationKind::derived,
        postproject::MediaSource::orderedParts(
            {{moved_media_path, "org.postproject:essence", false}}));
    if (optional_part.has_value() ||
        optional_part.error().code() !=
            postproject::ErrorCode::invalid_argument) {
      return 66;
    }
    additions.commit().value();

    const auto strip = reopened.representations(strip_id).value();
    if (strip.size() != 1 ||
        strip[0].kind != postproject::RepresentationKind::original ||
        strip[0].structureKind() !=
            postproject::ContentStructureKind::image_sequence ||
        strip[0].imageSequence() == nullptr ||
        strip[0].resources.size() != 1 ||
        strip[0].resources[0].locators.size() != 1 ||
        strip[0].resources[0].locators[0].sequence_naming !=
            postproject::SequenceNaming{"frame", ".exr", 4} ||
        strip[0].imageSequence()->start != 1 ||
        strip[0].imageSequence()->end != 1) {
      return 67;
    }

    const auto added_representations = reopened.representations(asset_id).value();
    const auto has_representation = [&](const postproject::RepresentationId &id,
                                        postproject::ContentStructureKind kind) {
      return std::any_of(
          added_representations.begin(), added_representations.end(),
          [&](const postproject::Representation &representation) {
            return representation.id == id &&
                   representation.structureKind() == kind;
          });
    };
    if (added_representations.size() != 5 ||
        !has_representation(
            proxy_id, postproject::ContentStructureKind::single_resource) ||
        !has_representation(
            sequence_id, postproject::ContentStructureKind::image_sequence) ||
        !has_representation(
            ordered_id, postproject::ContentStructureKind::ordered_parts) ||
        !has_representation(package_id,
                            postproject::ContentStructureKind::package)) {
      return 23;
    }

    const postproject::Dependency dependency{
        std::nullopt,
        "org.postproject:reference.character",
        asset_ref,
        resolutions[0].representation_id,
        true,
        "characters/lead.pproj#character/A"};
    auto dependency_update = reopened.readSession().value().edit().value();
    dependency_update.recordDependencySet(proxy_id, {dependency}).value();
    dependency_update.commit().value();
    const auto dependency_revision = reopened.latestRevision().value();
    if (!dependency_revision.has_value()) {
      return 32;
    }
    const auto dependency_events =
        reopened.revisionEvents(dependency_revision->id).value();
    const auto dependencies = reopened.dependencySet(proxy_id).value();
    const auto dependency_matches =
        reopened.dependencies(proxy_id, 4, 1000, 1).value();
    const auto dependents = reopened.dependents(asset_ref, 1, 1000, 1000).value();
    if (!dependencies.has_value() ||
        dependencies->source_representation_id != proxy_id ||
        dependencies->recorded_at_revision == 0 ||
        dependencies->status != postproject::DependencySetStatus::current ||
        dependencies->dependencies.size() != 1 ||
        dependencies->dependencies[0].source_resource_id.has_value() ||
        dependencies->dependencies[0].kind != dependency.kind ||
        !(dependencies->dependencies[0].target == dependency.target) ||
        dependencies->dependencies[0].resolved_representation_id !=
            dependency.resolved_representation_id ||
        !dependencies->dependencies[0].required ||
        dependencies->dependencies[0].authored_reference !=
            dependency.authored_reference ||
        dependency_matches.items.size() != 1 ||
        !(dependency_matches.items[0].target == asset_ref) ||
        dependency_matches.items[0].depth != 1 ||
        dependency_matches.next_cursor.has_value() ||
        dependency_matches.traversal_truncated ||
        dependents.items.size() != 1 ||
        dependents.items[0].target.representationId().value() != proxy_id ||
        dependents.items[0].target.kind() != postproject::ObjectKind::representation ||
        dependents.items[0].depth != 1 || dependents.next_cursor.has_value() ||
        dependents.traversal_truncated ||
        dependency_events.size() != 1 ||
        !std::holds_alternative<postproject::DependencySetRecordedEvent>(
            dependency_events[0].payload) ||
        std::get<postproject::DependencySetRecordedEvent>(
            dependency_events[0].payload)
                .representation_id != proxy_id) {
      return 31;
    }

    auto job_request = reopened.beginTransaction().value();
    const auto job_id = job_request.requestJob(
        {"org.postproject:generate-proxy", {resolutions[0].representation_id},
         asset_id, postproject::RepresentationKind::proxy, std::nullopt}).value();
    job_request.commit().value();
    const auto jobs = reopened.jobs(1000).value();
    if (jobs.items.size() != 1 || jobs.items[0].id != job_id ||
        jobs.items[0].kind != "org.postproject:generate-proxy" ||
        jobs.items[0].inputs !=
            std::vector<postproject::RepresentationId>{resolutions[0].representation_id} ||
        jobs.items[0].output_asset_id != asset_id ||
        jobs.items[0].output_representation_kind !=
            postproject::RepresentationKind::proxy ||
        jobs.items[0].target_root.has_value() ||
        jobs.items[0].stateKind() != postproject::JobState::requested ||
        std::holds_alternative<postproject::JobClaim>(jobs.items[0].status) ||
        std::holds_alternative<postproject::JobCompletion>(jobs.items[0].status) ||
        std::holds_alternative<postproject::JobFailure>(jobs.items[0].status) ||
        jobs.next_cursor.has_value()) {
      return 33;
    }
    const auto point_job = reopened.job(job_id).value();
    if (point_job.id != job_id ||
        point_job.stateKind() != postproject::JobState::requested) {
      return 60;
    }

    auto claim = reopened.beginTransaction().value();
    auto lease = claim.claimJobLease(
        job_id, {"C++ worker", std::string("1.0"), std::nullopt}, std::chrono::minutes(1),
        postproject::AgentIdentity{std::string("operator"), std::nullopt}).value();
    claim.commit().value();
    const auto claimed_jobs = reopened.jobs(1000).value();
    if (claimed_jobs.items.size() != 1 ||
        !std::holds_alternative<postproject::JobClaim>(claimed_jobs.items[0].status) ||
        claimed_jobs.items[0].stateKind() != postproject::JobState::claimed ||
        std::get<postproject::JobClaim>(claimed_jobs.items[0].status).tool.name != "C++ worker" ||
        std::get<postproject::JobClaim>(claimed_jobs.items[0].status).tool.version != std::string("1.0") ||
        !std::get<postproject::JobClaim>(claimed_jobs.items[0].status).agent.has_value() ||
        std::get<postproject::JobClaim>(claimed_jobs.items[0].status).agent->name != std::string("operator") ||
        std::get<postproject::JobClaim>(claimed_jobs.items[0].status).expires_at_unix_micros <= 0) {
      return 34;
    }

    auto renew = reopened.beginTransaction().value();
    renew.renewJobLease(lease, std::chrono::minutes(2)).value();
    renew.commit().value();
    auto release = reopened.beginTransaction().value();
    release.releaseJobLease(lease).value();
    release.commit().value();

    auto second_claim = reopened.beginTransaction().value();
    auto second_lease = second_claim.claimJobLease(
        job_id, {"C++ worker", std::nullopt, std::nullopt}, std::chrono::minutes(1)).value();
    second_claim.commit().value();
    auto fail = reopened.beginTransaction().value();
    fail.failJobLease(second_lease, "encoder exited").value();
    fail.commit().value();

    auto second_request = reopened.beginTransaction().value();
    const auto cancelled_job_id = second_request.requestJob(
        {"org.postproject:generate-thumbnail",
         {resolutions[0].representation_id}, asset_id,
         postproject::RepresentationKind::derived, std::nullopt}).value();
    second_request.commit().value();
    auto cancel = reopened.beginTransaction().value();
    cancel.cancelJob(cancelled_job_id).value();
    cancel.commit().value();

    const auto final_jobs = reopened.jobs(1000).value();
    const auto failed_job = std::find_if(
        final_jobs.items.begin(), final_jobs.items.end(),
        [&](const postproject::Job &candidate) { return candidate.id == job_id; });
    const auto cancelled_job = std::find_if(
        final_jobs.items.begin(), final_jobs.items.end(),
        [&](const postproject::Job &candidate) {
          return candidate.id == cancelled_job_id;
        });
    if (final_jobs.items.size() != 2 ||
        failed_job == final_jobs.items.end() ||
        failed_job->stateKind() != postproject::JobState::failed ||
        std::holds_alternative<postproject::JobClaim>(failed_job->status) ||
        std::get<postproject::JobFailure>(failed_job->status).diagnostic != std::string("encoder exited") ||
        cancelled_job == final_jobs.items.end() ||
        cancelled_job->stateKind() != postproject::JobState::cancelled) {
      return 35;
    }

    auto completion_request = reopened.beginTransaction().value();
    const auto completed_job_id = completion_request.requestJob(
        {"org.postproject:generate-proxy",
         {resolutions[0].representation_id}, asset_id,
         postproject::RepresentationKind::proxy, std::nullopt}).value();
    completion_request.commit().value();
    auto completion_claim = reopened.beginTransaction().value();
    auto completion_lease = completion_claim.claimJobLease(
        completed_job_id, {"C++ worker", std::nullopt, std::nullopt}, std::chrono::minutes(1)).value();
    completion_claim.commit().value();
    auto completion = reopened.beginTransaction().value();
    const auto completed_representation_id =
        completion.addRepresentation(
            asset_id, postproject::RepresentationKind::proxy,
            moved_media_path).value();
    const auto completion_activity_id = completion.createActivity(
        {"org.postproject:transcode",
         std::nullopt,
         std::nullopt,
         postproject::ToolIdentity{"C++ worker", std::nullopt, std::nullopt},
         std::nullopt,
         {{resolutions[0].representation_id,
           std::string("org.postproject:input.primary-video")}},
         {{completed_representation_id,
           std::string("org.postproject:output.proxy")}}}).value();
    completion.completeJobLease(completion_lease,
                           completed_representation_id,
                           completion_activity_id).value();
    completion.commit().value();

    const auto completed_jobs = reopened.jobs(1000).value();
    const auto completed_job = std::find_if(
        completed_jobs.items.begin(), completed_jobs.items.end(),
        [&](const postproject::Job &candidate) {
          return candidate.id == completed_job_id;
        });
    const auto completion_producing =
        reopened.activitiesProducing(completed_representation_id).value();
    if (completed_jobs.items.size() != 3 ||
        completed_job == completed_jobs.items.end() ||
        completed_job->stateKind() != postproject::JobState::succeeded ||
        !std::holds_alternative<postproject::JobCompletion>(completed_job->status) ||
        std::get<postproject::JobCompletion>(completed_job->status).activity_id != completion_activity_id ||
        std::get<postproject::JobCompletion>(completed_job->status).representation_id !=
            completed_representation_id ||
        completion_producing.size() != 1 ||
        completion_producing[0].id != completion_activity_id ||
        !completion_producing[0].outputs[0].snapshot.has_value()) {
      return 36;
    }
    const auto journal = reopened.changesSince(0, 1000).value();
    const auto journaled = [&](auto event_tag) {
      using Event = typename decltype(event_tag)::type;
      for (const auto &revision : journal) {
        for (const auto &event : reopened.revisionEvents(revision.id).value()) {
          if (std::holds_alternative<Event>(event.payload)) {
            return true;
          }
        }
      }
      return false;
    };
    if (!journaled(EventTag<postproject::JobRequestedEvent>{}) ||
        !journaled(EventTag<postproject::JobClaimedEvent>{}) ||
        !journaled(EventTag<postproject::JobClaimRenewedEvent>{}) ||
        !journaled(EventTag<postproject::JobClaimReleasedEvent>{}) ||
        !journaled(EventTag<postproject::JobFailedEvent>{}) ||
        !journaled(EventTag<postproject::JobCancelledEvent>{}) ||
        !journaled(EventTag<postproject::JobSucceededEvent>{})) {
      return 57;
    }

    const auto consuming_page =
        reopened.activitiesConsuming(resolutions[0].representation_id, 1).value();
    const auto tool_outputs = reopened.outputsByTool(
        {"C++ worker", std::nullopt, std::nullopt}, 10).value();
    const auto versioned_tool_outputs = reopened.outputsByTool(
        {"C++ worker", std::string("1.0"), std::nullopt}, 10).value();
    const auto ancestor_page =
        reopened.ancestors(completed_representation_id, 4, 1000, 10).value();
    const auto descendant_page =
        reopened.descendants(resolutions[0].representation_id, 4, 1000, 10).value();
    if (consuming_page.items.size() != 1 ||
        consuming_page.items[0].id != completion_activity_id ||
        consuming_page.next_cursor.has_value() ||
        tool_outputs.items !=
            std::vector<postproject::RepresentationId>{completed_representation_id} ||
        tool_outputs.next_cursor.has_value() ||
        !versioned_tool_outputs.items.empty() ||
        ancestor_page.items.size() != 1 ||
        !(ancestor_page.items[0].object ==
          postproject::ObjectRef::representation(resolutions[0].representation_id)) ||
        ancestor_page.items[0].depth != 1 ||
        ancestor_page.next_cursor.has_value() ||
        ancestor_page.traversal_truncated ||
        descendant_page.items.size() != 1 ||
        descendant_page.items[0].object.representationId().value() != completed_representation_id ||
        descendant_page.items[0].depth != 1 ||
        descendant_page.next_cursor.has_value() ||
        descendant_page.traversal_truncated) {
      return 48;
    }

    const auto all_representations = reopened.representations(asset_id).value();
    std::vector<postproject::RepresentationId> paged_representation_ids;
    std::optional<std::string> representation_cursor;
    do {
      const auto page =
          reopened.representations(asset_id, 2, representation_cursor).value();
      if (page.items.empty() || page.items.size() > 2) {
        return 49;
      }
      for (const auto &representation : page.items) {
        paged_representation_ids.push_back(representation.id);
      }
      representation_cursor = page.next_cursor;
    } while (representation_cursor.has_value());
    if (paged_representation_ids.size() != all_representations.size() ||
        !std::all_of(all_representations.begin(), all_representations.end(),
                     [&](const postproject::Representation &representation) {
                       return std::find(paged_representation_ids.begin(),
                                        paged_representation_ids.end(),
                                        representation.id) !=
                              paged_representation_ids.end();
                     })) {
      return 49;
    }

    const auto first_job_page = reopened.jobs(1).value();
    if (first_job_page.items.size() != 1 ||
        !first_job_page.next_cursor.has_value()) {
      return 38;
    }
    const auto second_job_page =
        reopened.jobs(1, *first_job_page.next_cursor).value();
    const auto succeeded_jobs =
        reopened.jobs(1000, std::nullopt,
                      postproject::JobState::succeeded).value();
    if (second_job_page.items.size() != 1 ||
        second_job_page.items[0].id == first_job_page.items[0].id ||
        succeeded_jobs.items.size() != 1 ||
        succeeded_jobs.items[0].id != completed_job_id ||
        succeeded_jobs.next_cursor.has_value()) {
      return 38;
    }

    const auto regeneration_plans = reopened.planRegeneration(
        {resolutions[0].representation_id, resolutions[0].representation_id}).value();
    if (regeneration_plans.size() != 1 ||
        regeneration_plans[0].artifact_representation_id !=
            resolutions[0].representation_id ||
        regeneration_plans[0].job.kind != "org.postproject:ingest" ||
        !regeneration_plans[0].job.inputs.empty() ||
        regeneration_plans[0].job.output_asset_id != asset_id ||
        regeneration_plans[0].job.output_representation_kind !=
            postproject::RepresentationKind::original ||
        regeneration_plans[0].job.stateKind() !=
            postproject::JobState::requested ||
        regeneration_plans[0].parameters.size() != 1 ||
        regeneration_plans[0].parameters[0].vocabulary !=
            "com.example.ingest" ||
        regeneration_plans[0].parameters[0].property != "details" ||
        reopened.jobs(1000).value().items.size() != 3) {
      return 37;
    }

    const auto stale_before = reopened.staleArtifacts(64, 1000, 100).value();
    if (!stale_before.items.empty() || stale_before.next_cursor.has_value() ||
        stale_before.traversal_truncated) {
      return 50;
    }
    auto invalidation = reopened.readSession().value().edit().value();
    invalidation.recordRepresentationFingerprint(
        resolutions[0].representation_id,
        {"cpp-smoke-tree", 1, {UINT8_C(0x31), UINT8_C(0x41)}}).value();
    invalidation.commit().value();
    const auto stale_all = reopened.staleArtifacts(64, 1000, 100).value();
    const auto stale_downstream = reopened.staleArtifacts(
        64, 1000, 100, std::nullopt, resolutions[0].representation_id).value();
    const auto contains_id = [](const std::vector<postproject::RepresentationId> &ids,
                                const postproject::RepresentationId &id) {
      return std::find(ids.begin(), ids.end(), id) != ids.end();
    };
    if (!contains_id(stale_all.items, completed_representation_id) ||
        stale_all.next_cursor.has_value() ||
        stale_downstream.items !=
            std::vector<postproject::RepresentationId>{completed_representation_id} ||
        stale_downstream.next_cursor.has_value() ||
        stale_downstream.traversal_truncated) {
      return 51;
    }

    {
      std::filesystem::remove(path + ".waits");
      auto watched = postproject::Production::create(path + ".waits").value();
      auto waiter = watched.revisionWaiter().value();
      if (waiter.wait(0, 10, std::chrono::milliseconds(0)).value().result !=
          postproject::RevisionWaitResult::timed_out) {
        return 52;
      }
      std::mutex delivered_mutex;
      std::condition_variable delivered_changed;
      std::vector<std::uint64_t> delivered;
      postproject::RevisionObserver observer(
          watched, 0,
          [&](const postproject::Revision &revision,
              const std::vector<postproject::RevisionEvent> &events) {
            if (events.empty() ||
                !std::holds_alternative<postproject::MediaRootAddedEvent>(
                    events[0].payload)) {
              return;
            }
            const std::lock_guard<std::mutex> lock(delivered_mutex);
            delivered.push_back(revision.sequence);
            delivered_changed.notify_all();
          },
          {postproject::RevisionEventKind::media_root_added});
      auto root_transaction = watched.beginTransaction().value();
      static_cast<void>(root_transaction.addMediaRoot("watched").value());
      root_transaction.commit().value();
      {
        std::unique_lock<std::mutex> lock(delivered_mutex);
        if (!delivered_changed.wait_for(lock, std::chrono::seconds(60),
                                        [&] { return !delivered.empty(); }) ||
            delivered != std::vector<std::uint64_t>{1}) {
          return 53;
        }
      }
      observer.stop();
      if (observer.error().has_value() || observer.cursor() != 1) {
        return 54;
      }
      const auto filtered = watched.changesSinceFiltered(
          0, {postproject::RevisionEventKind::media_root_added,
              postproject::RevisionEventKind::job_failed}).value();
      const auto waited = waiter.wait(0, 10).value();
      if (filtered.revisions.size() != 1 || filtered.through_sequence != 1 ||
          waited.result != postproject::RevisionWaitResult::revisions ||
          waited.revisions.size() != 1 ||
          waited.revisions[0].id != filtered.revisions[0].id) {
        return 55;
      }
      waiter.cancel();
      if (waiter.wait(1, 10).value().result !=
          postproject::RevisionWaitResult::cancelled) {
        return 56;
      }
    }

    try {
      static_cast<void>(postproject::Production::open(path + ".missing").value());
      return 6;
    } catch (const postproject::Exception &error) {
      if (error.code() == postproject::ErrorCode::ok ||
          std::string(error.what()).empty()) {
        return 7;
      }
    }
    const int renamed = renamed_sequence_scenario(path);
    if (renamed != 0) {
      return renamed;
    }
  } catch (const std::exception &error) {
    std::fprintf(stderr, "%s\n", error.what());
    return 1;
  }

  return 0;
}
