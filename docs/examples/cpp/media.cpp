// Runs the C++ listings about representations, media roots, and locators.
//
// Each "[name]" ... "[/name]" region is included verbatim by the documentation
// build, so keep regions self-contained and readable. Usage:
//   postproject-cpp-media WORK_DIRECTORY
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
#include <type_traits>
#include <variant>
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

// [add-representation]
postproject::Uuid add_proxy(postproject::Production &production,
                            const postproject::Uuid &asset_id,
                            const std::string &proxy_path) {
  auto transaction = production.beginTransaction().value();
  const auto proxy_id = transaction.addSingleFileRepresentation(
      asset_id, postproject::RepresentationKind::proxy, proxy_path).value();
  transaction.commit().value();
  return proxy_id;
}
// [/add-representation]

// [ordered-parts]
postproject::Uuid add_spanned_clip(postproject::Production &production,
                                   const postproject::Uuid &asset_id,
                                   const std::string &directory) {
  // Parts are stored in this order; every part of a span is required.
  const std::vector<postproject::FileResourceInput> parts{
      {directory + "/CLIP0001.MTS", "org.postproject:essence", true},
      {directory + "/CLIP0002.MTS", "org.postproject:span-part", true},
  };
  auto transaction = production.beginTransaction().value();
  const auto clip_id = transaction.addOrderedPartsRepresentation(
      asset_id, postproject::RepresentationKind::original, parts).value();
  transaction.commit().value();
  return clip_id;
}
// [/ordered-parts]

// [package-representation]
postproject::Uuid add_package(postproject::Production &production,
                              const postproject::Uuid &asset_id,
                              const std::string &directory) {
  // A package needs at least one required member; sidecars may be optional.
  const std::vector<postproject::FileResourceInput> members{
      {directory + "/clip.mxf", "org.postproject:essence", true},
      {directory + "/clip.xml", "org.postproject:sidecar", false},
  };
  auto transaction = production.beginTransaction().value();
  const auto package_id = transaction.addPackageRepresentation(
      asset_id, postproject::RepresentationKind::original, members).value();
  transaction.commit().value();
  return package_id;
}
// [/package-representation]

// [representation-structure]
std::vector<postproject::Representation>
print_structure(const postproject::Production &production,
                const postproject::Uuid &asset_id) {
  std::vector<postproject::Representation> all;
  std::optional<std::string> cursor;
  do {
    const auto page = production.representations(asset_id, 2, cursor).value();
    for (const auto &representation : page.items) {
      std::cout << "representation kind "
                << static_cast<std::uint32_t>(representation.kind)
                << ", structure "
                << static_cast<std::uint32_t>(representation.structure_kind)
                << ", " << representation.fingerprints.size()
                << " fingerprint(s)\n";
      for (const auto &member : representation.members) {
        std::cout << "  member " << member.role.value_or("-")
                  << (member.required ? " (required)" : " (optional)") << '\n';
      }
      for (const auto &resource : representation.resources) {
        for (const auto &fingerprint : resource.fingerprints) {
          std::cout << "  resource fingerprint " << fingerprint.algorithm
                    << " v" << fingerprint.version << '\n';
        }
        for (const auto &locator : resource.locators) {
          std::cout << "  locator " << locator.uri << '\n';
        }
      }
      if (const auto &sequence = representation.image_sequence) {
        std::cout << "  frames " << sequence->start << '-' << sequence->end
                  << " step " << sequence->step << ", missing:";
        for (const auto frame : sequence->missing_frames) {
          std::cout << ' ' << frame;
        }
        std::cout << '\n';
      }
      all.push_back(representation);
    }
    cursor = page.next_cursor;
  } while (cursor.has_value());
  return all;
}
// [/representation-structure]

// [media-root-lifecycle]
void cycle_media_root(postproject::Production &production,
                      const std::string &name) {
  std::optional<postproject::Uuid> root_id;
  for (const auto &root : production.mediaRoots().value()) {
    std::cout << "root " << root.name << " priority " << root.priority
              << (root.enabled ? " enabled" : " disabled") << '\n';
    if (root.name == name) {
      root_id = root.id;
    }
  }
  if (!root_id) {
    return;
  }

  auto disable = production.beginTransaction().value();
  // A disabled root is kept but skipped during resolution.
  disable.setMediaRootEnabled(*root_id, false).value();
  disable.commit().value();

  auto enable = production.beginTransaction().value();
  enable.setMediaRootEnabled(*root_id, true).value();
  enable.commit().value();

  auto remove = production.beginTransaction().value();
  remove.removeMediaRoot(*root_id).value();
  remove.commit().value();
}
// [/media-root-lifecycle]

// [verify-resolution]
std::size_t verify_contents(const postproject::Production &production,
                            const postproject::Uuid &asset_id) {
  // Content mode re-fingerprints files at known locators instead of trusting
  // their presence.
  postproject::ResolutionOptions options;
  options.setVerification(postproject::VerificationMode::content);
  std::size_t verified = 0;
  for (const auto &representation :
       production.resolveAsset(asset_id, options).value()) {
    for (const auto &resource : representation.resources) {
      if (resource.state ==
          postproject::ResourceResolutionState::online_at_known_locator) {
        ++verified;
      } else if (resource.state ==
                 postproject::ResourceResolutionState::error) {
        // fingerprint_mismatch evidence: the content was replaced.
        std::cout << "content differs for " << resource.evidence.size()
                  << " reason(s)\n";
      }
    }
  }
  return verified;
}
// [/verify-resolution]

// [resolve-scope]
postproject::Result<std::optional<std::string>>
find_nearby(const postproject::Production &production,
            const std::vector<postproject::Uuid> &asset_ids,
            const std::string &directory,
            const postproject::CancelToken &cancel_token) {
  // A search directory is an unnamed, machine-local place such as the project
  // folder or where the media used to be; it is never recorded. Each searched
  // directory has its own budget, and another thread may cancel the token.
  postproject::ResolutionOptions options;
  options.addSearchDirectory(directory)
      .setVerification(postproject::VerificationMode::presence)
      .setLimits(16, 50000)
      .setCancelToken(cancel_token);
  // All assets are resolved together; each directory is scanned once. A
  // cancelled token yields ErrorCode::cancelled.
  auto resolutions = production.resolveAssets(asset_ids, options);
  if (!resolutions.ok()) {
    return resolutions.error();
  }
  for (const auto &representation : *resolutions) {
    for (const auto &resource : representation.resources) {
      const bool discovered =
          resource.state == postproject::ResourceResolutionState::resolved_exact ||
          resource.state ==
              postproject::ResourceResolutionState::resolved_probable;
      // A candidate from a search directory has no media root.
      if (discovered && resource.candidates.size() == 1 &&
          !resource.candidates.front().media_root.has_value()) {
        return std::optional<std::string>(resource.candidates.front().uri);
      }
    }
  }
  return std::optional<std::string>();
}
// [/resolve-scope]

// [retire-locator]
std::vector<postproject::ResourceLocator> move_resource(
    postproject::Production &production, const postproject::Uuid &resource_id,
    const postproject::Uuid &old_locator_id, const std::string &new_uri) {
  auto transaction = production.beginTransaction().value();
  transaction.confirmLocator(resource_id, new_uri).value();
  // Retiring keeps the old locator as history instead of deleting it.
  transaction.retireLocator(old_locator_id).value();
  transaction.commit().value();

  std::vector<postproject::ResourceLocator> locators;
  std::optional<std::string> cursor;
  do {
    const auto page = production.locators(resource_id, 1, cursor).value();
    locators.insert(locators.end(), page.items.begin(), page.items.end());
    cursor = page.next_cursor;
  } while (cursor.has_value());
  return locators;
}
// [/retire-locator]

// [locator-uri]
bool is_recorded_locator(const std::string &path,
                         const std::string &recorded_uri) {
  // Spell the path as PostProject spells locators instead of building a URI
  // with the host's own URL type, then compare the strings exactly.
  std::cout << "recorded locator " << recorded_uri << " is "
            << postproject::locatorFilePath(recorded_uri).value() << '\n';
  return postproject::fileLocator(path).value() == recorded_uri;
}
// [/locator-uri]

// [content-fingerprint]
void print_file_fingerprint(const std::string &path) {
  // The same value import records; computing it records nothing.
  const postproject::Fingerprint fingerprint = postproject::fingerprintFile(path).value();
  std::cout << fingerprint.algorithm << " v" << fingerprint.version << ": "
            << fingerprint.value.size() << " bytes\n";
}
// [/content-fingerprint]

// [fingerprint-observation]
std::vector<postproject::RevisionEvent>
observe_new_content(postproject::Production &production,
                    const postproject::Uuid &resource_id,
                    const std::string &path) {
  // Verification only reads: it compares the file with the stored value.
  if (production.verifyResource(resource_id, path).value() !=
      postproject::ContentVerification::differs) {
    throw std::runtime_error("content is unchanged");
  }

  // Stages the new resource fingerprint and every representation fingerprint
  // recomputed from it; commit records both in one revision.
  auto transaction = production.beginTransaction().value();
  transaction.observeResourceContent(resource_id, path).value();
  transaction.commit().value();

  const auto events =
      production.revisionEvents(production.latestRevision().value()->id).value();
  for (const auto &event : events) {
    std::visit(
        [](const auto &payload) {
          using Payload = std::decay_t<decltype(payload)>;
          if constexpr (std::is_same_v<
                            Payload,
                            postproject::ResourceFingerprintObservedEvent>) {
            std::cout << "resource fingerprint observed: " << payload.algorithm
                      << '\n';
          } else if constexpr (
              std::is_same_v<
                  Payload,
                  postproject::RepresentationFingerprintObservedEvent>) {
            std::cout << "representation fingerprint observed: "
                      << payload.algorithm << '\n';
          }
        },
        event.payload);
  }
  return events;
}
// [/fingerprint-observation]

// [resolution-issues]
std::vector<postproject::AvailabilityIssue>
report_issues(const postproject::Production &production,
              const postproject::Uuid &asset_id) {
  std::vector<postproject::AvailabilityIssue> all;
  for (const auto &representation : production.resolveAsset(asset_id).value()) {
    for (const auto &issue : representation.issues) {
      std::cout << "issue " << static_cast<std::uint32_t>(issue.kind)
                << (issue.required ? " (required)" : "") << ", frames:";
      for (const auto frame : issue.frames) {
        std::cout << ' ' << frame;
      }
      std::cout << '\n';
      all.push_back(issue);
    }
    for (const auto &resource : representation.resources) {
      std::cout << "  resource state "
                << static_cast<std::uint32_t>(resource.state) << '\n';
      for (const auto &evidence : resource.evidence) {
        std::cout << "  evidence " << static_cast<std::uint32_t>(evidence.kind)
                  << ": " << evidence.detail.value_or("-") << '\n';
      }
    }
  }
  return all;
}
// [/resolution-issues]

postproject::Uuid add_sequence(postproject::Production &production,
                               const postproject::Uuid &asset_id,
                               const std::string &directory) {
  postproject::ImageSequenceInput sequence{};
  sequence.directory = directory;
  sequence.prefix = "shot010.";
  sequence.suffix = ".exr";
  sequence.padding = 4;
  sequence.start = 1001;
  sequence.end = 1004;
  sequence.step = 1;
  sequence.rate_numerator = 24;
  sequence.rate_denominator = 1;
  sequence.missing_frames = {1003};
  auto transaction = production.beginTransaction().value();
  const auto id = transaction.addImageSequenceRepresentation(
      asset_id, postproject::RepresentationKind::derived, sequence).value();
  transaction.commit().value();
  return id;
}

const postproject::Representation &
find(const std::vector<postproject::Representation> &representations,
     const postproject::Uuid &id) {
  const auto found = std::find_if(
      representations.begin(), representations.end(),
      [&](const postproject::Representation &item) { return item.id == id; });
  require(found != representations.end(), "representation listed");
  return *found;
}

} // namespace

int main(int argc, char **argv) {
  if (argc != 2) {
    std::cerr << "usage: postproject-cpp-media WORK_DIRECTORY\n";
    return 2;
  }
  const std::filesystem::path work = argv[1];
  const std::string media = (work / "rushes" / "A001.mov").string();

  try {
    write_file(work / "proxies" / "A001_proxy.mov", "proxy bytes");
    write_file(work / "proxies" / "moved" / "A001_proxy.mov", "proxy bytes");
    write_file(work / "spanned" / "CLIP0001.MTS", "first span part");
    write_file(work / "spanned" / "CLIP0002.MTS", "second span part");
    write_file(work / "package" / "clip.mxf", "package essence");
    write_file(work / "package" / "clip.xml", "<clip/>");

    auto production = postproject::Production::create(
        (work / "media.pproj").string(), "Media").value();
    auto setup = production.beginTransaction().value();
    const auto asset_id = setup.importMedia(media, "Camera A").value();
    setup.addMediaRoot("rushes", "Camera originals", 10).value();
    setup.addMediaRoot("archive", 0).value();
    setup.commit().value();
    const auto original_id = production.representations(asset_id).value().front().id;

    const auto proxy_id = add_proxy(
        production, asset_id, (work / "proxies" / "A001_proxy.mov").string());
    const auto clip_id =
        add_spanned_clip(production, asset_id, (work / "spanned").string());
    const auto package_id =
        add_package(production, asset_id, (work / "package").string());
    const auto sequence_id = add_sequence(
        production, asset_id, (work / "renders" / "shot010").string());

    const auto representations = print_structure(production, asset_id);
    require(representations.size() == 5, "five representations paged");
    require(find(representations, proxy_id).kind ==
                postproject::RepresentationKind::proxy,
            "proxy kind");
    const auto &clip = find(representations, clip_id);
    require(clip.structure_kind ==
                    postproject::ContentStructureKind::ordered_parts &&
                clip.members.size() == 2 && clip.resources.size() == 2,
            "ordered parts");
    const auto &package = find(representations, package_id);
    require(package.structure_kind ==
                    postproject::ContentStructureKind::package &&
                package.members.size() == 2 && package.members[0].required &&
                !package.members[1].required,
            "package members");
    const auto &sequence = find(representations, sequence_id);
    require(sequence.image_sequence.has_value() &&
                sequence.image_sequence->missing_frames ==
                    std::vector<std::int64_t>{1003},
            "sequence descriptor");

    cycle_media_root(production, "archive");
    const auto roots = production.mediaRoots().value();
    require(roots.size() == 1 && roots.front().name == "rushes" &&
                roots.front().enabled,
            "archive root removed");

    const auto &proxy = find(representations, proxy_id);
    const auto &proxy_resource = proxy.resources.front();
    require(proxy_resource.locators.size() == 1, "one proxy locator");
    const auto &old_locator = proxy_resource.locators.front();
    // The proxy moved; find it where it went instead of spelling a URI.
    std::filesystem::remove(work / "proxies" / "A001_proxy.mov");
    postproject::CancelToken cancel_token;
    const auto found = find_nearby(production, {asset_id},
                                   (work / "proxies" / "moved").string(),
                                   cancel_token)
                           .value();
    require(found.has_value(), "moved proxy found in the search directory");
    const std::string new_uri = *found;
    cancel_token.cancel();
    const auto cancelled = find_nearby(
        production, {asset_id}, (work / "proxies" / "moved").string(),
        cancel_token);
    require(!cancelled.ok() &&
                cancelled.error().code() == postproject::ErrorCode::cancelled,
            "cancelled resolution reports cancelled");
    const auto locators =
        move_resource(production, proxy_resource.id, old_locator.id, new_uri);
    require(std::any_of(locators.begin(), locators.end(),
                        [&](const postproject::ResourceLocator &match) {
                          return match.locator.uri == new_uri;
                        }),
            "new locator listed");
    const auto events_before = production.latestRevision().value()->sequence;

    write_file(media, "re-exported camera original");
    const auto &original = find(representations, original_id);
    require(is_recorded_locator(
                media, original.resources.front().locators.front().uri),
            "recorded locator matches the path");
    print_file_fingerprint(media);
    const auto events =
        observe_new_content(production, original.resources.front().id, media);
    require(production.latestRevision().value()->sequence == events_before + 1,
            "one observation revision");
    require(events.size() == 2 &&
                std::holds_alternative<
                    postproject::ResourceFingerprintObservedEvent>(
                    events[0].payload) &&
                std::holds_alternative<
                    postproject::RepresentationFingerprintObservedEvent>(
                    events[1].payload),
            "fingerprint events");
    const auto observed = production.representations(asset_id).value();
    require(find(observed, original_id).fingerprints.front().value !=
                original.fingerprints.front().value,
            "recomputed representation fingerprint");
    require(production.verifyResource(original.resources.front().id, media).value() ==
                postproject::ContentVerification::matches,
            "observed content verifies");

    require(verify_contents(production, asset_id) > 0,
            "content verification checks known locators");
    const auto issues = report_issues(production, asset_id);
    require(
        std::any_of(
            issues.begin(), issues.end(),
            [](const postproject::AvailabilityIssue &issue) {
              return issue.kind ==
                         postproject::AvailabilityIssueKind::missing_frames &&
                     issue.frames == std::vector<std::int64_t>{1003};
            }),
        "missing frame issue");
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
