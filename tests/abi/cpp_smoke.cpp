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

int main(int argc, char **argv) {
  if (argc != 2) {
    return 2;
  }

  const std::string path(argv[1]);
  std::remove(path.c_str());

  try {
    if (postproject::abi_version() != 32) {
      return 3;
    }

    auto production = postproject::Production::create(path, "C++ smoke test");
    const auto created_id = production.id();
    const std::string media_path = path + ".media";
    {
      std::ofstream media(media_path, std::ios::binary);
      media << "C++ transaction media";
      if (!media) {
        return 10;
      }
    }
    auto transaction = production.beginTransaction();
    transaction.setRevisionContext(
        {postproject::OriginIdentity{"C++ smoke", std::string("1.0"),
                                     std::nullopt},
         std::string("Import fixture")});
    const auto asset_id = transaction.importMedia(media_path, "C++ asset");
    const postproject::ObjectRef asset_ref{postproject::ObjectKind::asset,
                                           asset_id};
    const postproject::HostObjectBinding host_binding{created_id, asset_ref};
    const auto host_binding_text = host_binding.toString();
    if (host_binding_text.rfind("https://postproject.org/ref/v1/", 0) != 0 ||
        !(postproject::HostObjectBinding::fromString(host_binding_text) ==
          host_binding)) {
      return 21;
    }
    const postproject::ExternalIdentifier external_id{
        "com.example.asset", "asset-42", std::string("primary")};
    transaction.addExternalIdentifier(asset_ref, external_id);
    const auto root_id = transaction.addMediaRoot("fixtures", "Fixture media");
    transaction.commit();
    const auto latest_revision = production.latestRevision();
    const auto revision_page = production.changesSince(0, 1);
    if (!latest_revision.has_value() || latest_revision->sequence != 1 ||
        !latest_revision->origin.has_value() ||
        latest_revision->origin->name != "C++ smoke" ||
        latest_revision->origin->version != std::string("1.0") ||
        latest_revision->message != std::string("Import fixture") ||
        revision_page.size() != 1 ||
        revision_page[0].id != latest_revision->id) {
      return 15;
    }
    const auto revision_events = production.revisionEvents(latest_revision->id);
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
    if (!production.containsAsset(asset_id)) {
      return 8;
    }
    const auto assets = production.assets();
    if (assets.size() != 1 || assets[0].id != asset_id ||
        assets[0].created_at_unix_micros == 0 ||
        assets[0].display_name != std::string("C++ asset") ||
        assets[0].import_source.has_value()) {
      return 24;
    }
    const auto asset_page = production.assets(1);
    if (asset_page.items.size() != 1 || asset_page.items[0].id != asset_id ||
        asset_page.items[0].display_name != std::string("C++ asset") ||
        asset_page.next_cursor.has_value() || asset_page.traversal_truncated) {
      return 40;
    }
    const auto roots = production.mediaRoots();
    if (roots.size() != 1 || roots[0].id != root_id ||
        roots[0].name != "fixtures" ||
        roots[0].label != std::string("Fixture media") ||
        roots[0].legacy_uri.has_value() || roots[0].priority != 0 ||
        !roots[0].enabled) {
      return 25;
    }
    const auto representations = production.representations(asset_id);
    if (representations.size() != 1 ||
        representations[0].asset_id != asset_id ||
        representations[0].kind != postproject::RepresentationKind::original ||
        representations[0].structure_kind !=
            postproject::ContentStructureKind::single_resource ||
        representations[0].members.size() != 1 ||
        !representations[0].members[0].required ||
        representations[0].members[0].role.has_value() ||
        representations[0].image_sequence.has_value() ||
        representations[0].fingerprints.size() != 1 ||
        representations[0].fingerprints[0].version != 1 ||
        representations[0].fingerprints[0].value.empty() ||
        representations[0].resources.size() != 1 ||
        representations[0].resources[0].id !=
            representations[0].members[0].resource_id ||
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
    if (production.dependencySet(representations[0].id).has_value() ||
        !production.dependents(asset_ref, 1, 1000, 1000).items.empty()) {
      return 30;
    }
    auto observations = production.beginTransaction();
    observations.recordResourceFingerprint(
        representations[0].resources[0].id,
        {"cpp-smoke", 1, {UINT8_C(0x10), UINT8_C(0x20)}});
    observations.recordRepresentationFingerprint(
        representations[0].id,
        {"cpp-smoke-tree", 1, {UINT8_C(0x30), UINT8_C(0x40)}});
    observations.commit();
    const auto observation_revision = production.latestRevision();
    if (!observation_revision.has_value()) {
      return 28;
    }
    const auto observation_events =
        production.revisionEvents(observation_revision->id);
    if (observation_events.size() != 2 ||
        !std::holds_alternative<postproject::ResourceFingerprintObservedEvent>(
            observation_events[0].payload) ||
        !std::holds_alternative<
            postproject::RepresentationFingerprintObservedEvent>(
            observation_events[1].payload)) {
      return 29;
    }
    const auto observed_representations = production.representations(asset_id);
    if (observed_representations[0].fingerprints.size() != 2 ||
        observed_representations[0].resources[0].fingerprints.size() != 2) {
      return 26;
    }
    const auto computed = postproject::fingerprintFile(media_path);
    const auto &stored = representations[0].resources[0].fingerprints[0];
    if (computed.algorithm != stored.algorithm ||
        computed.version != stored.version || computed.value != stored.value ||
        production.verifyResource(representations[0].resources[0].id,
                                  media_path) !=
            postproject::ContentVerification::matches) {
      return 61;
    }
    const auto &locator_uri =
        representations[0].resources[0].locators[0].uri;
    if (postproject::fileLocator(media_path) != locator_uri ||
        postproject::fileLocator(postproject::locatorFilePath(locator_uri)) !=
            locator_uri) {
      return 62;
    }
    auto content_observation = production.beginTransaction();
    content_observation.observeResourceContent(
        representations[0].resources[0].id, media_path);
    content_observation.commit();
    const auto identifiers = production.externalIdentifiers(asset_ref);
    const auto found =
        production.findByExternalIdentifier("com.example.asset", "asset-42",
                                            external_id.qualifier);
    if (identifiers.size() != 1 ||
        identifiers[0].scheme != external_id.scheme ||
        identifiers[0].value != external_id.value ||
        identifiers[0].qualifier != external_id.qualifier || found.size() != 1 ||
        !(found[0] == asset_ref)) {
      return 13;
    }

    auto rolled_back = production.beginTransaction();
    const auto discarded_id = rolled_back.importMedia(media_path);
    rolled_back.rollback();
    if (production.containsAsset(discarded_id)) {
      return 9;
    }

    const std::string moved_media_path = media_path + ".moved";
    std::filesystem::rename(media_path, moved_media_path);
    const std::string fixtures =
        std::filesystem::path(path).parent_path().string();
    postproject::ResolutionOptions options;
    options.addRootMapping("fixtures", fixtures)
        .addSearchDirectory(fixtures)
        .setVerification(postproject::VerificationMode::presence)
        .setLimits(64, 1000000);
    {
      postproject::CancelToken token;
      options.setCancelToken(token);
    }
    const auto resolutions = production.resolveAssets({asset_id}, options);
    if (resolutions.size() != 1 || !(resolutions[0].asset_id == asset_id) ||
        resolutions[0].resources[0].candidates.empty() ||
        resolutions[0].resources[0].candidates[0].media_root !=
            std::optional<std::string>("fixtures") ||
        resolutions[0].availability !=
            postproject::RepresentationAvailability::online ||
        resolutions[0].resources.size() != 1 ||
        resolutions[0].resources[0].state !=
            postproject::ResourceResolutionState::resolved_exact ||
        resolutions[0].resources[0].candidates.size() != 1 ||
        resolutions[0]
                .resources[0]
                .candidates[0]
                .confidence_basis_points != 10000 ||
        resolutions[0].resources[0].candidates[0].evidence.empty()) {
      return 11;
    }
    postproject::CancelToken cancel_token;
    options.setCancelToken(cancel_token);
    cancel_token.cancel();
    try {
      (void)production.resolveAsset(asset_id, options);
      return 63;
    } catch (const postproject::Error &error) {
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
    const auto pre_provenance_revision = production.latestRevision();
    if (!pre_provenance_revision.has_value()) {
      return 41;
    }
    auto provenance = production.beginTransaction();
    const auto activity_id = provenance.createActivity(activity_spec);
    std::vector<postproject::MetadataInput> metadata_items;
    metadata_items.push_back(postproject::MetadataInput::rational(24000, 1001));
    metadata_items.push_back(
        postproject::MetadataInput::languageString("Interview", "en-US"));
    std::vector<postproject::MetadataFieldInput> metadata_fields;
    metadata_fields.push_back(
        {"values", postproject::MetadataInput::list(metadata_items)});
    const auto metadata =
        postproject::MetadataInput::structure(metadata_fields);
    provenance.addMetadataValue(
        {postproject::ObjectKind::activity, activity_id},
        "com.example.ingest", "details", metadata);
    provenance.addMetadataValue(asset_ref, "com.example.ingest", "title",
                                postproject::MetadataInput::plainString(
                                    "Interview"));
    provenance.commit();

    const auto title_matches = production.queryMetadata(
        "com.example.ingest", "title",
        postproject::MetadataInput::plainString("Interview"), 10);
    const auto title_misses = production.queryMetadata(
        "com.example.ingest", "title",
        postproject::MetadataInput::plainString("Other"), 10);
    const auto detail_matches =
        production.queryMetadata("com.example.ingest", "details", 1);
    if (title_matches.items.size() != 1 ||
        !(title_matches.items[0].target == asset_ref) ||
        title_matches.items[0].vocabulary != "com.example.ingest" ||
        title_matches.items[0].property != "title" ||
        title_matches.next_cursor.has_value() ||
        !title_misses.items.empty() || title_misses.next_cursor.has_value() ||
        detail_matches.items.size() != 1 ||
        !(detail_matches.items[0].target ==
          postproject::ObjectRef{postproject::ObjectKind::activity,
                                 activity_id}) ||
        detail_matches.next_cursor.has_value()) {
      return 42;
    }
    try {
      static_cast<void>(production.queryMetadata(
          "com.example.ingest", "details", metadata, 10));
      return 43;
    } catch (const postproject::Error &error) {
      if (error.code() != postproject::ErrorCode::invalid_argument) {
        return 43;
      }
    }

    std::vector<postproject::ObjectRef> changed_objects;
    std::optional<std::string> changed_cursor;
    do {
      const auto changed = production.objectsChangedSince(
          pre_provenance_revision->sequence, 1, changed_cursor);
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
        !contains_object({postproject::ObjectKind::activity, activity_id})) {
      return 44;
    }

    const auto activities = production.activities();
    const auto producing =
        production.activitiesProducing(resolutions[0].representation_id);
    const auto producing_page =
        production.activitiesProducing(resolutions[0].representation_id, 1);
    const auto ingest_outputs =
        production.outputsByActivityKind("org.postproject:ingest", 1);
    if (producing_page.items.size() != 1 ||
        producing_page.items[0].id != activity_id ||
        producing_page.items[0].kind != "org.postproject:ingest" ||
        producing_page.next_cursor.has_value() ||
        ingest_outputs.items !=
            std::vector<postproject::Uuid>{resolutions[0].representation_id} ||
        ingest_outputs.next_cursor.has_value() ||
        ingest_outputs.traversal_truncated ||
        !production.unresolvedMedia(100).items.empty()) {
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
        !production.ancestors(resolutions[0].representation_id).empty()) {
      return 14;
    }
    const auto artifact =
        production.evaluateArtifact(resolutions[0].representation_id);
    const auto reproducibility =
        production.artifactReproducibility(resolutions[0].representation_id);
    if (artifact.state != postproject::ArtifactKnowledgeState::current ||
        artifact.visited_representations != 1 || artifact.truncated ||
        !artifact.reasons.empty() || !reproducibility.reproducible ||
        reproducibility.producing_activity_id != activity_id ||
        reproducibility.activity_kind !=
            std::string("org.postproject:ingest") ||
        !reproducibility.issues.empty()) {
      return 28;
    }
    auto confirmation = production.beginTransaction();
    confirmation.confirmLocatorUnderRoot(
        resolutions[0].resources[0].resource_id,
        resolutions[0].resources[0].candidates[0].uri, "fixtures");
    confirmation.setMediaRootEnabled(root_id, false);
    confirmation.retireLocator(
        representations[0].resources[0].locators[0].id);
    confirmation.commit();

    const auto rooted = production.representationsUnderMediaRoot("fixtures", 1);
    const auto resource_page =
        production.resources(resolutions[0].representation_id, 1);
    const auto locator_page =
        production.locators(resolutions[0].resources[0].resource_id, 10);
    const auto rooted_locator = std::find_if(
        locator_page.items.begin(), locator_page.items.end(),
        [&](const postproject::ResourceLocator &candidate) {
          return candidate.locator.uri ==
                 resolutions[0].resources[0].candidates[0].uri;
        });
    if (rooted.items.size() != 1 ||
        rooted.items[0].id != resolutions[0].representation_id ||
        rooted.items[0].resources.size() != 1 ||
        rooted.next_cursor.has_value() ||
        resource_page.items !=
            std::vector<postproject::Uuid>{
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
                              std::string_view("not-a-cursor")));
      return 47;
    } catch (const postproject::Error &error) {
      if (error.code() != postproject::ErrorCode::invalid_argument) {
        return 47;
      }
    }
    const auto point_asset = production.asset(asset_id);
    const auto point_representation =
        production.representation(resolutions[0].representation_id);
    const auto resource_users = production.representationsUsingResource(
        resolutions[0].resources[0].resource_id, 10);
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
          production.representation(postproject::Uuid(postproject::Uuid::Bytes{})));
      return 59;
    } catch (const postproject::Error &error) {
      if (error.code() != postproject::ErrorCode::not_found) {
        return 59;
      }
    }
    const auto disabled_roots = production.mediaRoots();
    if (disabled_roots.size() != 1 || disabled_roots[0].enabled) {
      return 26;
    }
    auto root_removal = production.beginTransaction();
    root_removal.removeMediaRoot(root_id);
    root_removal.commit();
    if (!production.mediaRoots().empty()) {
      return 27;
    }

    auto moved = std::move(production);
    if (production || !moved) {
      return 4;
    }

    auto reopened = postproject::Production::open(path);
    if (reopened.id() != created_id || !reopened.containsAsset(asset_id)) {
      return 5;
    }
    const auto persisted = reopened.resolveAsset(asset_id);
    if (persisted.size() != 1 ||
        persisted[0].availability !=
            postproject::RepresentationAvailability::online ||
        persisted[0].resources.size() != 1 ||
        persisted[0].resources[0].state !=
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
    auto additions = reopened.beginTransaction();
    const auto proxy_id = additions.addSingleFileRepresentation(
        asset_id, postproject::RepresentationKind::proxy, moved_media_path);
    const auto sequence_id = additions.addImageSequenceRepresentation(
        asset_id, postproject::RepresentationKind::derived,
        {sequence_frame_path.parent_path().string(), "frame", ".exr", 4, 1,
         1, 1, 24000, 1001, {}});
    const std::vector<postproject::FileResourceInput> ordered_members{
        {moved_media_path, "org.postproject:essence.first", true},
        {sequence_frame_path.string(), "org.postproject:essence.second", true},
    };
    const auto ordered_id = additions.addOrderedPartsRepresentation(
        asset_id, postproject::RepresentationKind::optimized, ordered_members);
    const std::vector<postproject::FileResourceInput> package_members{
        {moved_media_path, "org.postproject:essence", true},
        {sequence_frame_path.string(), "org.postproject:sidecar", false},
    };
    const auto package_id = additions.addPackageRepresentation(
        asset_id, postproject::RepresentationKind::derived, package_members);
    additions.commit();

    const auto added_representations = reopened.representations(asset_id);
    const auto has_representation = [&](const postproject::Uuid &id,
                                        postproject::ContentStructureKind kind) {
      return std::any_of(
          added_representations.begin(), added_representations.end(),
          [&](const postproject::Representation &representation) {
            return representation.id == id &&
                   representation.structure_kind == kind;
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
    auto dependency_update = reopened.beginTransaction();
    dependency_update.recordDependencySet(proxy_id, {dependency});
    dependency_update.commit();
    const auto dependency_revision = reopened.latestRevision();
    if (!dependency_revision.has_value()) {
      return 32;
    }
    const auto dependency_events =
        reopened.revisionEvents(dependency_revision->id);
    const auto dependencies = reopened.dependencySet(proxy_id);
    const auto dependency_matches =
        reopened.dependencies(proxy_id, 4, 1000, 1);
    const auto dependents = reopened.dependents(asset_ref, 1, 1000, 1000);
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
        dependents.items[0].target.id != proxy_id ||
        dependents.items[0].target.kind != postproject::ObjectKind::representation ||
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

    auto job_request = reopened.beginTransaction();
    const auto job_id = job_request.requestJob(
        {"org.postproject:generate-proxy", {resolutions[0].representation_id},
         asset_id, postproject::RepresentationKind::proxy, std::nullopt});
    job_request.commit();
    const auto jobs = reopened.jobs(1000);
    if (jobs.items.size() != 1 || jobs.items[0].id != job_id ||
        jobs.items[0].kind != "org.postproject:generate-proxy" ||
        jobs.items[0].inputs !=
            std::vector<postproject::Uuid>{resolutions[0].representation_id} ||
        jobs.items[0].output_asset_id != asset_id ||
        jobs.items[0].output_representation_kind !=
            postproject::RepresentationKind::proxy ||
        jobs.items[0].target_root.has_value() ||
        jobs.items[0].state != postproject::JobState::requested ||
        jobs.items[0].claim.has_value() ||
        jobs.items[0].completion.has_value() ||
        jobs.items[0].failure_diagnostic.has_value() ||
        jobs.next_cursor.has_value()) {
      return 33;
    }
    const auto point_job = reopened.job(job_id);
    if (point_job.id != job_id ||
        point_job.state != postproject::JobState::requested) {
      return 60;
    }

    auto claim = reopened.beginTransaction();
    const auto claim_id = claim.claimJob(
        job_id, {"C++ worker", std::string("1.0"), std::nullopt},
        postproject::AgentIdentity{std::string("operator"), std::nullopt}, 10,
        20);
    claim.commit();
    const auto claimed_jobs = reopened.jobs(1000);
    if (claimed_jobs.items.size() != 1 ||
        !claimed_jobs.items[0].claim.has_value() ||
        claimed_jobs.items[0].state != postproject::JobState::claimed ||
        claimed_jobs.items[0].claim->id != claim_id ||
        claimed_jobs.items[0].claim->tool.name != "C++ worker" ||
        claimed_jobs.items[0].claim->tool.version != std::string("1.0") ||
        !claimed_jobs.items[0].claim->agent.has_value() ||
        claimed_jobs.items[0].claim->agent->name != std::string("operator") ||
        claimed_jobs.items[0].claim->expires_at_unix_micros != 20) {
      return 34;
    }

    auto renew = reopened.beginTransaction();
    renew.renewJobClaim(job_id, claim_id, 11, 30);
    renew.commit();
    auto release = reopened.beginTransaction();
    release.releaseJobClaim(job_id, claim_id);
    release.commit();

    auto second_claim = reopened.beginTransaction();
    const auto second_claim_id = second_claim.claimJob(
        job_id, {"C++ worker", std::nullopt, std::nullopt}, std::nullopt, 31,
        40);
    second_claim.commit();
    auto fail = reopened.beginTransaction();
    fail.failJob(job_id, second_claim_id, 32, "encoder exited");
    fail.commit();

    auto second_request = reopened.beginTransaction();
    const auto cancelled_job_id = second_request.requestJob(
        {"org.postproject:generate-thumbnail",
         {resolutions[0].representation_id}, asset_id,
         postproject::RepresentationKind::derived, std::nullopt});
    second_request.commit();
    auto cancel = reopened.beginTransaction();
    cancel.cancelJob(cancelled_job_id);
    cancel.commit();

    const auto final_jobs = reopened.jobs(1000);
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
        failed_job->state != postproject::JobState::failed ||
        failed_job->claim.has_value() ||
        failed_job->failure_diagnostic != std::string("encoder exited") ||
        cancelled_job == final_jobs.items.end() ||
        cancelled_job->state != postproject::JobState::cancelled) {
      return 35;
    }

    auto completion_request = reopened.beginTransaction();
    const auto completed_job_id = completion_request.requestJob(
        {"org.postproject:generate-proxy",
         {resolutions[0].representation_id}, asset_id,
         postproject::RepresentationKind::proxy, std::nullopt});
    completion_request.commit();
    auto completion_claim = reopened.beginTransaction();
    const auto completion_claim_id = completion_claim.claimJob(
        completed_job_id, {"C++ worker", std::nullopt, std::nullopt},
        std::nullopt, 41, 50);
    completion_claim.commit();
    auto completion = reopened.beginTransaction();
    const auto completed_representation_id =
        completion.addSingleFileRepresentation(
            asset_id, postproject::RepresentationKind::proxy,
            moved_media_path);
    const auto completion_activity_id = completion.createActivity(
        {"org.postproject:transcode",
         std::nullopt,
         std::nullopt,
         postproject::ToolIdentity{"C++ worker", std::nullopt, std::nullopt},
         std::nullopt,
         {{resolutions[0].representation_id,
           std::string("org.postproject:input.primary-video")}},
         {{completed_representation_id,
           std::string("org.postproject:output.proxy")}}});
    completion.completeJob(completed_job_id, completion_claim_id, 42,
                           completed_representation_id,
                           completion_activity_id);
    completion.commit();

    const auto completed_jobs = reopened.jobs(1000);
    const auto completed_job = std::find_if(
        completed_jobs.items.begin(), completed_jobs.items.end(),
        [&](const postproject::Job &candidate) {
          return candidate.id == completed_job_id;
        });
    const auto completion_producing =
        reopened.activitiesProducing(completed_representation_id);
    if (completed_jobs.items.size() != 3 ||
        completed_job == completed_jobs.items.end() ||
        completed_job->state != postproject::JobState::succeeded ||
        !completed_job->completion.has_value() ||
        completed_job->completion->activity_id != completion_activity_id ||
        completed_job->completion->representation_id !=
            completed_representation_id ||
        completion_producing.size() != 1 ||
        completion_producing[0].id != completion_activity_id ||
        !completion_producing[0].outputs[0].snapshot.has_value()) {
      return 36;
    }
    const auto journal = reopened.changesSince(0, 1000);
    const auto journaled = [&](auto event_tag) {
      using Event = typename decltype(event_tag)::type;
      for (const auto &revision : journal) {
        for (const auto &event : reopened.revisionEvents(revision.id)) {
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
        reopened.activitiesConsuming(resolutions[0].representation_id, 1);
    const auto tool_outputs = reopened.outputsByTool(
        {"C++ worker", std::nullopt, std::nullopt}, 10);
    const auto versioned_tool_outputs = reopened.outputsByTool(
        {"C++ worker", std::string("1.0"), std::nullopt}, 10);
    const auto ancestor_page =
        reopened.ancestors(completed_representation_id, 4, 1000, 10);
    const auto descendant_page =
        reopened.descendants(resolutions[0].representation_id, 4, 1000, 10);
    if (consuming_page.items.size() != 1 ||
        consuming_page.items[0].id != completion_activity_id ||
        consuming_page.next_cursor.has_value() ||
        tool_outputs.items !=
            std::vector<postproject::Uuid>{completed_representation_id} ||
        tool_outputs.next_cursor.has_value() ||
        !versioned_tool_outputs.items.empty() ||
        ancestor_page.items.size() != 1 ||
        !(ancestor_page.items[0].object ==
          postproject::ObjectRef{postproject::ObjectKind::representation,
                                 resolutions[0].representation_id}) ||
        ancestor_page.items[0].depth != 1 ||
        ancestor_page.next_cursor.has_value() ||
        ancestor_page.traversal_truncated ||
        descendant_page.items.size() != 1 ||
        descendant_page.items[0].object.id != completed_representation_id ||
        descendant_page.items[0].depth != 1 ||
        descendant_page.next_cursor.has_value() ||
        descendant_page.traversal_truncated) {
      return 48;
    }

    const auto all_representations = reopened.representations(asset_id);
    std::vector<postproject::Uuid> paged_representation_ids;
    std::optional<std::string> representation_cursor;
    do {
      const auto page =
          reopened.representations(asset_id, 2, representation_cursor);
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

    const auto first_job_page = reopened.jobs(1);
    if (first_job_page.items.size() != 1 ||
        !first_job_page.next_cursor.has_value()) {
      return 38;
    }
    const auto second_job_page =
        reopened.jobs(1, *first_job_page.next_cursor);
    const auto succeeded_jobs =
        reopened.jobs(1000, std::nullopt,
                      postproject::JobState::succeeded);
    if (second_job_page.items.size() != 1 ||
        second_job_page.items[0].id == first_job_page.items[0].id ||
        succeeded_jobs.items.size() != 1 ||
        succeeded_jobs.items[0].id != completed_job_id ||
        succeeded_jobs.next_cursor.has_value()) {
      return 38;
    }

    const auto regeneration_plans = reopened.planRegeneration(
        {resolutions[0].representation_id, resolutions[0].representation_id});
    if (regeneration_plans.size() != 1 ||
        regeneration_plans[0].artifact_representation_id !=
            resolutions[0].representation_id ||
        regeneration_plans[0].job.kind != "org.postproject:ingest" ||
        !regeneration_plans[0].job.inputs.empty() ||
        regeneration_plans[0].job.output_asset_id != asset_id ||
        regeneration_plans[0].job.output_representation_kind !=
            postproject::RepresentationKind::original ||
        regeneration_plans[0].job.state !=
            postproject::JobState::requested ||
        regeneration_plans[0].parameters.size() != 1 ||
        regeneration_plans[0].parameters[0].vocabulary !=
            "com.example.ingest" ||
        regeneration_plans[0].parameters[0].property != "details" ||
        reopened.jobs(1000).items.size() != 3) {
      return 37;
    }

    const auto stale_before = reopened.staleArtifacts(64, 1000, 100);
    if (!stale_before.items.empty() || stale_before.next_cursor.has_value() ||
        stale_before.traversal_truncated) {
      return 50;
    }
    auto invalidation = reopened.beginTransaction();
    invalidation.recordRepresentationFingerprint(
        resolutions[0].representation_id,
        {"cpp-smoke-tree", 1, {UINT8_C(0x31), UINT8_C(0x41)}});
    invalidation.commit();
    const auto stale_all = reopened.staleArtifacts(64, 1000, 100);
    const auto stale_downstream = reopened.staleArtifacts(
        64, 1000, 100, std::nullopt, resolutions[0].representation_id);
    const auto contains_id = [](const std::vector<postproject::Uuid> &ids,
                                const postproject::Uuid &id) {
      return std::find(ids.begin(), ids.end(), id) != ids.end();
    };
    if (!contains_id(stale_all.items, completed_representation_id) ||
        stale_all.next_cursor.has_value() ||
        stale_downstream.items !=
            std::vector<postproject::Uuid>{completed_representation_id} ||
        stale_downstream.next_cursor.has_value() ||
        stale_downstream.traversal_truncated) {
      return 51;
    }

    {
      std::filesystem::remove(path + ".waits");
      auto watched = postproject::Production::create(path + ".waits");
      auto waiter = watched.revisionWaiter();
      if (waiter.wait(0, 10, std::chrono::milliseconds(0)).result !=
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
      auto root_transaction = watched.beginTransaction();
      static_cast<void>(root_transaction.addMediaRoot("watched"));
      root_transaction.commit();
      {
        std::unique_lock<std::mutex> lock(delivered_mutex);
        if (!delivered_changed.wait_for(lock, std::chrono::seconds(60),
                                        [&] { return !delivered.empty(); }) ||
            delivered != std::vector<std::uint64_t>{1}) {
          return 53;
        }
      }
      observer.stop();
      if (observer.error() != nullptr || observer.cursor() != 1) {
        return 54;
      }
      const auto filtered = watched.changesSinceFiltered(
          0, {postproject::RevisionEventKind::media_root_added,
              postproject::RevisionEventKind::job_failed});
      const auto waited = waiter.wait(0, 10);
      if (filtered.revisions.size() != 1 || filtered.through_sequence != 1 ||
          waited.result != postproject::RevisionWaitResult::revisions ||
          waited.revisions.size() != 1 ||
          waited.revisions[0].id != filtered.revisions[0].id) {
        return 55;
      }
      waiter.cancel();
      if (waiter.wait(1, 10).result !=
          postproject::RevisionWaitResult::cancelled) {
        return 56;
      }
    }

    try {
      static_cast<void>(postproject::Production::open(path + ".missing"));
      return 6;
    } catch (const postproject::Error &error) {
      if (error.code() == postproject::ErrorCode::ok ||
          std::string(error.what()).empty()) {
        return 7;
      }
    }
  } catch (const std::exception &error) {
    std::fprintf(stderr, "%s\n", error.what());
    return 1;
  }

  return 0;
}
