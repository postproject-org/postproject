// Runs the C++ listings about external identifiers and typed metadata.
//
// Each "[name]" ... "[/name]" region is included verbatim by the documentation
// build, so keep regions self-contained and readable. Usage:
//   postproject-cpp-knowledge WORK_DIRECTORY
// The work directory is prepared by prepare-workdir.cmake.
#include <postproject/postproject.hpp>

#include <cstdint>
#include <iostream>
#include <map>
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

// [remove-identifier]
std::vector<postproject::ExternalIdentifier>
replace_reel_name(postproject::Production &production,
                  const postproject::Uuid &asset_id) {
  const postproject::ObjectRef target{postproject::ObjectKind::asset, asset_id};
  const postproject::ExternalIdentifier reel{"com.example.reel", "A001",
                                             std::nullopt};
  const postproject::ExternalIdentifier serial{"com.example.camera.serial",
                                               "A-0007", std::string("body")};

  auto attach = production.beginTransaction();
  attach.addExternalIdentifier(target, reel);
  attach.addExternalIdentifier(target, serial);
  attach.commit();

  for (const auto &identifier : production.externalIdentifiers(target)) {
    std::cout << identifier.scheme << " = " << identifier.value << '\n';
  }
  for (const auto &match :
       production.findByExternalIdentifier(reel.scheme, reel.value,
                                           reel.qualifier)) {
    std::cout << "reel A001 names object kind "
              << static_cast<std::uint32_t>(match.kind) << '\n';
  }

  // Removal needs the exact scheme, value, and qualifier that were attached.
  auto detach = production.beginTransaction();
  detach.removeExternalIdentifier(target, reel);
  detach.commit();
  return production.externalIdentifiers(target);
}
// [/remove-identifier]

// [typed-metadata]
constexpr const char *editorial = "https://example.com/ns/editorial/1";

void add_editorial_metadata(postproject::Production &production,
                            const postproject::Uuid &asset_id,
                            const postproject::Uuid &representation_id) {
  using postproject::MetadataInput;
  const postproject::ObjectRef asset{postproject::ObjectKind::asset, asset_id};

  std::vector<MetadataInput> keywords;
  keywords.push_back(MetadataInput::plainString("interview"));
  keywords.push_back(MetadataInput::plainString("exterior"));
  std::vector<postproject::MetadataFieldInput> slate;
  slate.push_back({"scene", MetadataInput::plainString("12A")});
  slate.push_back({"take", MetadataInput::unsignedInteger(3)});

  auto transaction = production.beginTransaction();
  const auto add = [&](const char *property, const MetadataInput &value) {
    transaction.addMetadataValue(asset, editorial, property, value);
  };
  add("title", MetadataInput::plainString("Harbour interview"));
  add("caption", MetadataInput::languageString("Am Hafen", "de-DE"));
  add("timecode-offset", MetadataInput::signedInteger(-48));
  add("frame-count", MetadataInput::unsignedInteger(86400));
  add("aspect-ratio", MetadataInput::decimal("239", 2)); // 2.39
  add("circled", MetadataInput::boolean(true));
  add("shot-at", MetadataInput::timestamp(1'700'000'000'000'000));
  add("licence", MetadataInput::uri("https://example.com/licences/7"));
  add("checksum", MetadataInput::bytes({0xde, 0xad, 0xbe, 0xef}));
  add("frame-rate", MetadataInput::rational(24000, 1001));
  add("keywords", MetadataInput::list(keywords));
  add("slate", MetadataInput::structure(slate));
  add("preferred-representation",
      MetadataInput::reference(
          {postproject::ObjectKind::representation, representation_id}));
  transaction.commit();
}

std::map<std::string, std::size_t>
count_editorial_values(const postproject::Production &production,
                       const std::vector<std::string> &properties) {
  std::map<std::string, std::size_t> counts;
  for (const auto &property : properties) {
    // Follow next_cursor until the query is exhausted.
    std::optional<std::string> cursor;
    do {
      const auto page =
          production.queryMetadata(editorial, property, 1, cursor);
      counts[property] += page.items.size();
      cursor = page.next_cursor;
    } while (cursor.has_value());
  }
  // Values come back as reusable MetadataInput objects: pass one to an
  // exact-value query, or copy it onto another target.
  const auto frame_rate = production.queryMetadata(
      editorial, "frame-rate",
      postproject::MetadataInput::rational(24000, 1001), 10);
  std::cout << "assets shot at 23.976 fps: " << frame_rate.items.size() << '\n';
  return counts;
}
// [/typed-metadata]

} // namespace

int main(int argc, char **argv) {
  if (argc != 2) {
    std::cerr << "usage: postproject-cpp-knowledge WORK_DIRECTORY\n";
    return 2;
  }
  const std::string work = argv[1];

  try {
    auto production =
        postproject::Production::create(work + "/knowledge.pproj", "Knowledge");
    auto setup = production.beginTransaction();
    const auto asset_id = setup.importMedia(work + "/rushes/A001.mov");
    setup.commit();
    const auto representation_id =
        production.representations(asset_id).front().id;
    const postproject::ObjectRef asset{postproject::ObjectKind::asset,
                                       asset_id};

    const auto remaining = replace_reel_name(production, asset_id);
    require(remaining.size() == 1 &&
                remaining.front().scheme == "com.example.camera.serial" &&
                remaining.front().qualifier ==
                    std::optional<std::string>("body"),
            "one identifier left");
    require(
        production.findByExternalIdentifier("com.example.reel", "A001").empty(),
        "removed identifier no longer matches");
    require(production.findByExternalIdentifier("com.example.camera.serial",
                                                "A-0007", "body") ==
                std::vector{asset},
            "serial still matches its qualifier");

    add_editorial_metadata(production, asset_id, representation_id);
    const std::vector<std::string> properties{"title",
                                              "caption",
                                              "timecode-offset",
                                              "frame-count",
                                              "aspect-ratio",
                                              "circled",
                                              "shot-at",
                                              "licence",
                                              "checksum",
                                              "frame-rate",
                                              "keywords",
                                              "slate",
                                              "preferred-representation"};
    const auto counts = count_editorial_values(production, properties);
    for (const auto &property : properties) {
      require(counts.at(property) == 1, "one value per property");
    }

    using postproject::MetadataInput;
    const auto exact = [&](const char *property, const MetadataInput &value) {
      const auto page =
          production.queryMetadata(editorial, property, value, 10);
      return page.items.size() == 1 && page.items.front().target == asset;
    };
    require(exact("title", MetadataInput::plainString("Harbour interview")),
            "string round trip");
    require(
        exact("caption", MetadataInput::languageString("Am Hafen", "de-DE")),
        "language string round trip");
    require(exact("timecode-offset", MetadataInput::signedInteger(-48)),
            "i64 round trip");
    require(exact("frame-count", MetadataInput::unsignedInteger(86400)),
            "u64 round trip");
    require(exact("aspect-ratio", MetadataInput::decimal("239", 2)),
            "decimal round trip");
    require(exact("circled", MetadataInput::boolean(true)), "bool round trip");
    require(exact("shot-at", MetadataInput::timestamp(1'700'000'000'000'000)),
            "timestamp round trip");
    require(
        exact("licence", MetadataInput::uri("https://example.com/licences/7")),
        "URI round trip");
    require(exact("frame-rate", MetadataInput::rational(24000, 1001)),
            "rational round trip");
    require(
        exact("preferred-representation",
              MetadataInput::reference({postproject::ObjectKind::representation,
                                        representation_id})),
        "reference round trip");
    require(!exact("circled", MetadataInput::boolean(false)),
            "exact query rejects other values");
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
