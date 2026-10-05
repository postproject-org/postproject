// Runs the C++ listings about activities, artifact staleness, and dependencies.
//
// Each "[name]" ... "[/name]" region is included verbatim by the documentation
// build, so keep regions self-contained and readable. Usage:
//   postproject-cpp-provenance WORK_DIRECTORY
// The work directory is prepared by prepare-workdir.cmake.
#include <postproject/postproject.hpp>

#include <algorithm>
#include <cstdint>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <optional>
#include <stdexcept>
#include <string>
#include <vector>

namespace {

void require(bool condition, const char *message) {
  if (!condition) {
    throw std::runtime_error(message);
  }
}

void write_file(const std::filesystem::path &path, const std::string &bytes) {
  std::filesystem::create_directories(path.parent_path());
  std::ofstream stream(path, std::ios::binary | std::ios::trunc);
  stream << bytes;
  require(static_cast<bool>(stream), "write file");
}

void print_snapshot(
    const char *label,
    const std::optional<postproject::ActivityEdgeSnapshot> &snapshot) {
  if (!snapshot) {
    std::cout << "  " << label << ": no snapshot\n";
    return;
  }
  for (const auto &fingerprint : snapshot->fingerprints) {
    std::cout << "  " << label << " at revision " << snapshot->revision_sequence
              << ": " << fingerprint.algorithm << " v" << fingerprint.version
              << '\n';
  }
}

// [activity-snapshots]
postproject::Uuid record_transcode(postproject::Production &production,
                                   const postproject::Uuid &original_id,
                                   const postproject::Uuid &proxy_id) {
  postproject::ActivitySpec spec{};
  spec.kind = "org.postproject:transcode";
  spec.started_at_unix_micros = 1'700'000'000'000'000;
  spec.finished_at_unix_micros = 1'700'000'060'000'000;
  spec.inputs = {{original_id, "org.postproject:essence"}};
  spec.outputs = {{proxy_id, std::nullopt}};
  spec.tool = postproject::ToolIdentity{"Example Encoder", "5.1", std::nullopt};
  spec.agent = postproject::AgentIdentity{
      "render-node-04", postproject::ExternalIdentifier{"com.example.farm.node",
                                                        "04", std::nullopt}};

  auto transaction = production.beginTransaction().value();
  const auto activity_id = transaction.createActivity(spec).value();
  transaction.commit().value();

  // Every edge carries the fingerprints its representation had at creation.
  for (const auto &activity : production.activities().value()) {
    std::cout << activity.kind << " by "
              << (activity.tool ? activity.tool->name : "unknown tool")
              << " on "
              << (activity.agent ? activity.agent->name.value_or("?") : "?")
              << '\n';
    for (const auto &input : activity.inputs) {
      print_snapshot("input", input.snapshot);
    }
    for (const auto &output : activity.outputs) {
      print_snapshot("output", output.snapshot);
    }
  }

  const auto consumers = production.activitiesConsuming(original_id).value();
  std::cout << "activities reading the original: " << consumers.size() << '\n';
  // Whole-set traversal, then the bounded paged form of the same query.
  const auto derived = production.descendants(original_id).value();
  const auto page = production.descendants(original_id, 8, 1000, 100).value();
  std::cout << derived.size() << " descendant(s), first page "
            << page.items.size() << '\n';
  return activity_id;
}
// [/activity-snapshots]

// [stale-after-change]
postproject::ArtifactEvaluation evaluate_after_change(
    postproject::Production &production,
    const postproject::Representation &original,
    const postproject::Fingerprint &new_resource_fingerprint,
    const postproject::Fingerprint &new_representation_fingerprint,
    const postproject::Uuid &proxy_id) {
  // Record what the host observed after the original was re-exported.
  auto transaction = production.beginTransaction().value();
  transaction.recordResourceFingerprint(original.resources.front().id,
                                        new_resource_fingerprint).value();
  transaction.recordRepresentationFingerprint(original.id,
                                              new_representation_fingerprint).value();
  transaction.commit().value();

  const auto evaluation = production.evaluateArtifact(proxy_id).value();
  if (evaluation.state == postproject::ArtifactKnowledgeState::stale) {
    for (const auto &reason : evaluation.reasons) {
      std::cout << "stale because " << static_cast<std::uint32_t>(reason.kind)
                << '\n';
    }
  }

  // Reproducibility says whether the recorded activity suffices to redo it.
  const auto reproducibility = production.artifactReproducibility(proxy_id).value();
  for (const auto &issue : reproducibility.issues) {
    std::cout << "cannot reproduce: " << static_cast<std::uint32_t>(issue.kind)
              << '\n';
  }
  return evaluation;
}
// [/stale-after-change]

// [dependency-set]
std::vector<postproject::DependencyMatch> record_scene_dependencies(
    postproject::Production &production, const postproject::Uuid &scene_id,
    const std::vector<postproject::Dependency> &observed) {
  // A dependency set replaces the complete previous observation.
  auto transaction = production.beginTransaction().value();
  transaction.recordDependencySet(scene_id, observed).value();
  transaction.commit().value();

  if (const auto set = production.dependencySet(scene_id).value()) {
    std::cout << set->dependencies.size() << " dependencies, "
              << (set->status == postproject::DependencySetStatus::current
                      ? "current"
                      : "needs extraction")
              << '\n';
  }

  std::vector<postproject::DependencyMatch> matches;
  std::optional<std::string> cursor;
  do {
    const auto page = production.dependencies(scene_id, 4, 1000, 1, cursor).value();
    matches.insert(matches.end(), page.items.begin(), page.items.end());
    cursor = page.next_cursor;
  } while (cursor.has_value());
  return matches;
}
// [/dependency-set]

postproject::Representation find(const postproject::Production &production,
                                 const postproject::AssetId &asset_id,
                                 const postproject::Uuid &id) {
  for (auto &representation : production.representations(asset_id).value()) {
    if (representation.id == id) {
      return representation;
    }
  }
  throw std::runtime_error("representation not found");
}

// Stand-in for the digest the host's hasher computes for changed bytes.
postproject::Fingerprint changed(const postproject::Fingerprint &stored,
                                 std::uint8_t fill) {
  return {stored.algorithm, stored.version,
          std::vector<std::uint8_t>(stored.value.size(), fill)};
}

} // namespace

int main(int argc, char **argv) {
  if (argc != 2) {
    std::cerr << "usage: postproject-cpp-provenance WORK_DIRECTORY\n";
    return 2;
  }
  const std::filesystem::path work = argv[1];
  const std::string media = (work / "rushes" / "A001.mov").string();

  try {
    write_file(work / "proxies" / "A001_proxy.mov", "proxy bytes");
    write_file(work / "characters" / "lead.usd", "#usda 1.0");
    write_file(work / "scenes" / "harbour.usd", "#usda 1.0 scene");

    auto production = postproject::Production::create(
        (work / "provenance.pproj").string(), "Provenance").value();
    auto setup = production.beginTransaction().value();
    const auto asset_id = setup.importMedia(media, "Camera A").value();
    const auto lead_asset_id =
        setup.importMedia((work / "characters" / "lead.usd").string()).value();
    const auto scene_asset_id =
        setup.importMedia((work / "scenes" / "harbour.usd").string()).value();
    setup.commit().value();
    const auto original_id = production.representations(asset_id).value().front().id;
    const auto lead_id = production.representations(lead_asset_id).value().front().id;
    const auto scene_id = production.representations(scene_asset_id).value().front().id;
    auto proxy_setup = production.beginTransaction().value();
    const auto proxy_id = proxy_setup.addRepresentation(
        asset_id, postproject::RepresentationKind::proxy,
        (work / "proxies" / "A001_proxy.mov").string()).value();
    proxy_setup.commit().value();

    const auto activity_id =
        record_transcode(production, original_id, proxy_id);
    const auto activities = production.activities().value();
    require(activities.size() == 1 && activities.front().id == activity_id,
            "one activity");
    const auto &activity = activities.front();
    require(activity.tool &&
                activity.tool->version == std::optional<std::string>("5.1"),
            "tool identity");
    require(activity.agent && activity.agent->identifier &&
                activity.agent->identifier->value == "04",
            "agent identity");
    require(activity.inputs.size() == 1 && activity.inputs.front().snapshot &&
                !activity.inputs.front().snapshot->fingerprints.empty(),
            "input snapshot");
    require(activity.outputs.size() == 1 && activity.outputs.front().snapshot,
            "output snapshot");
    require(production.activitiesConsuming(original_id).value().size() == 1,
            "consuming activity");
    require(production.descendants(original_id).value() == std::vector{proxy_id},
            "whole-set descendants");
    const auto page = production.descendants(original_id, 8, 1000, 100).value();
    require(page.items.size() == 1 &&
                page.items.front().object.id == proxy_id &&
                page.items.front().depth == 1,
            "paged descendants");
    require(production.evaluateArtifact(proxy_id).value().state ==
                postproject::ArtifactKnowledgeState::current,
            "proxy current before the change");

    write_file(media, "re-exported camera original");
    const auto original = find(production, asset_id, original_id);
    const auto evaluation = evaluate_after_change(
        production, original,
        changed(original.resources.front().fingerprints.front(), 0x55),
        changed(original.fingerprints.front(), 0x66), proxy_id);
    require(evaluation.state == postproject::ArtifactKnowledgeState::stale,
            "proxy stale");
    require(std::any_of(
                evaluation.reasons.begin(), evaluation.reasons.end(),
                [](const postproject::ArtifactReason &reason) {
                  return reason.kind ==
                         postproject::ArtifactReasonKind::fingerprint_changed;
                }),
            "fingerprint changed reason");
    const auto reproducibility = production.artifactReproducibility(proxy_id).value();
    require(!reproducibility.reproducible &&
                std::any_of(
                    reproducibility.issues.begin(),
                    reproducibility.issues.end(),
                    [](const postproject::ArtifactReproducibilityIssue &issue) {
                      return issue.kind ==
                             postproject::ArtifactReproducibilityIssueKind::
                                 parameters_missing;
                    }),
            "missing parameters");

    const std::vector<postproject::Dependency> observed{
        {std::nullopt,
         "org.example:plate",
         postproject::ObjectRef::asset(asset_id),
         original_id,
         true,
         "rushes/A001.mov"},
        {std::nullopt,
         "org.example:character-reference",
         postproject::ObjectRef::asset(lead_asset_id),
         lead_id,
         false,
         "characters/lead.usd"},
    };
    const auto matches =
        record_scene_dependencies(production, scene_id, observed);
    require(matches.size() == 2, "two dependencies paged");
    const auto current = production.dependencySet(scene_id).value();
    require(current &&
                current->status == postproject::DependencySetStatus::current &&
                current->dependencies.size() == 2 &&
                current->dependencies[1].authored_reference ==
                    "characters/lead.usd",
            "current dependency set");

    auto observation = production.beginTransaction().value();
    const auto scene = find(production, scene_asset_id, scene_id);
    observation.recordRepresentationFingerprint(
        scene_id, changed(scene.fingerprints.front(), 0x77)).value();
    observation.commit().value();
    const auto after = production.dependencySet(scene_id).value();
    require(after && after->status ==
                         postproject::DependencySetStatus::needs_extraction,
            "dependency set needs extraction");
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
