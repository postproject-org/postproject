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
#include <type_traits>
#include <variant>
#include <vector>

namespace {

void require(bool condition, const char *message) {
  if (!condition) {
    throw std::runtime_error(message);
  }
}

// [known-media-adoption]
postproject::AssetId find_known_media(const std::string &production_path,
                                   const std::string &media_path) {
  // A second host or process opens the same production explicitly.
  const auto production = postproject::Production::open(production_path).value();
  const postproject::LocatorIdentity locator{
      postproject::fileLocator(media_path).value(), std::nullopt};
  const auto fingerprint = postproject::fingerprintFile(media_path).value();
  const auto by_locator =
      production.findKnownMediaByLocator(locator, 100).value();
  const auto by_content =
      production.findKnownMediaByFingerprint(fingerprint, 100).value();

  // Several results are candidates for the host to present, never a winner.
  require(by_locator.items.size() == 1 && by_content.items.size() == 1,
          "one known-media candidate");
  require(by_locator.items.front().asset_id ==
              by_content.items.front().asset_id,
          "locator and content identify the same asset");
  return by_locator.items.front().asset_id;
}
// [/known-media-adoption]

// [remove-identifier]
std::vector<postproject::ExternalIdentifier>
replace_reel_name(postproject::Production &production,
                  const postproject::AssetId &asset_id) {
  const postproject::ObjectRef target = postproject::ObjectRef::asset(asset_id);
  const postproject::ExternalIdentifier reel{"com.example.reel", "A001",
                                             std::nullopt};
  const postproject::ExternalIdentifier serial{"com.example.camera.serial",
                                               "A-0007", std::string("body")};

  auto attach = production.beginTransaction().value();
  attach.addExternalIdentifier(target, reel).value();
  attach.addExternalIdentifier(target, serial).value();
  attach.commit().value();

  for (const auto &identifier : production.externalIdentifiers(target).value()) {
    std::cout << identifier.scheme << " = " << identifier.value << '\n';
  }
  for (const auto &match :
       production.findByExternalIdentifier(reel.scheme, reel.value,
                                           reel.qualifier).value()) {
    std::cout << "reel A001 names object kind "
              << static_cast<std::uint32_t>(match.kind) << '\n';
  }

  // Removal needs the exact scheme, value, and qualifier that were attached.
  auto detach = production.beginTransaction().value();
  detach.removeExternalIdentifier(target, reel).value();
  detach.commit().value();
  return production.externalIdentifiers(target).value();
}
// [/remove-identifier]

// [typed-metadata]
constexpr const char *editorial = "https://example.com/ns/editorial/1";

void add_editorial_metadata(postproject::Production &production,
                            const postproject::AssetId &asset_id,
                            const postproject::Uuid &representation_id) {
  using postproject::MetadataValue;
  const postproject::ObjectRef asset = postproject::ObjectRef::asset(asset_id);

  std::vector<MetadataValue> keywords;
  keywords.push_back(MetadataValue::plainString("interview"));
  keywords.push_back(MetadataValue::plainString("exterior"));
  std::vector<postproject::MetadataField> slate;
  slate.push_back({"scene", MetadataValue::plainString("12A")});
  slate.push_back({"take", MetadataValue::unsignedInteger(3)});

  auto transaction = production.beginTransaction().value();
  const auto add = [&](const char *property, const MetadataValue &value) {
    transaction.addMetadataValue(asset, editorial, property, value).value();
  };
  add("title", MetadataValue::plainString("Harbour interview"));
  add("caption", MetadataValue::languageString("Am Hafen", "de-DE"));
  add("timecode-offset", MetadataValue::signedInteger(-48));
  add("frame-count", MetadataValue::unsignedInteger(86400));
  add("aspect-ratio", MetadataValue::decimal("239", 2)); // 2.39
  add("circled", MetadataValue::boolean(true));
  add("shot-at", MetadataValue::timestamp(1'700'000'000'000'000));
  add("licence", MetadataValue::uri("https://example.com/licences/7"));
  add("checksum", MetadataValue::bytes({0xde, 0xad, 0xbe, 0xef}));
  add("frame-rate", MetadataValue::rational(24000, 1001));
  add("keywords", MetadataValue::list(keywords));
  add("slate", MetadataValue::structure(slate));
  add("preferred-representation",
      MetadataValue::reference(
          {postproject::ObjectKind::representation, representation_id}));
  transaction.commit().value();
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
          production.queryMetadata(editorial, property, 1, cursor).value();
      counts[property] += page.items.size();
      cursor = page.next_cursor;
    } while (cursor.has_value());
  }
  // Values come back as MetadataValue objects: read them, pass one to an
  // exact-value query, or copy it onto another target.
  const auto frame_rate = production.queryMetadata(
      editorial, "frame-rate",
      postproject::MetadataValue::rational(24000, 1001), 10).value();
  std::cout << "assets shot at 23.976 fps: " << frame_rate.items.size() << '\n';

  const auto keywords =
      production.queryMetadata(editorial, "keywords", 1).value();
  if (const auto *list =
          keywords.items.front().value.getIf<postproject::MetadataList>()) {
    for (const auto &item : list->items) {
      std::cout << "keyword: "
                << item.getIf<postproject::MetadataString>()->value << '\n';
    }
  }
  const auto slate = production.queryMetadata(editorial, "slate", 1).value();
  std::visit(
      [](const auto &value) {
        using Value = std::decay_t<decltype(value)>;
        if constexpr (std::is_same_v<Value, postproject::MetadataStructure>) {
          std::cout << "slate has " << value.fields.size() << " fields\n";
        }
      },
      slate.items.front().value.variant());
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
    const std::string production_path = work + "/knowledge.pproj";
    const std::string media_path = work + "/rushes/A001.mov";
    auto production = postproject::Production::create(production_path, "Knowledge").value();
    auto setup = production.beginTransaction().value();
    const auto asset_id = setup.importMedia(media_path).value();
    setup.commit().value();
    require(find_known_media(production_path, media_path) == asset_id,
            "second handle found imported asset");
    const auto representation_id =
        production.representations(asset_id).value().front().id;
    const postproject::ObjectRef asset = postproject::ObjectRef::asset(asset_id);

    const auto remaining = replace_reel_name(production, asset_id);
    require(remaining.size() == 1 &&
                remaining.front().scheme == "com.example.camera.serial" &&
                remaining.front().qualifier ==
                    std::optional<std::string>("body"),
            "one identifier left");
    require(
        production.findByExternalIdentifier("com.example.reel", "A001").value().empty(),
        "removed identifier no longer matches");
    require(production.findByExternalIdentifier("com.example.camera.serial",
                                                "A-0007", "body").value() ==
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

    using postproject::MetadataValue;
    const auto exact = [&](const char *property, const MetadataValue &value) {
      const auto page =
          production.queryMetadata(editorial, property, value, 10).value();
      return page.items.size() == 1 && page.items.front().target == asset;
    };
    require(exact("title", MetadataValue::plainString("Harbour interview")),
            "string round trip");
    require(
        exact("caption", MetadataValue::languageString("Am Hafen", "de-DE")),
        "language string round trip");
    require(exact("timecode-offset", MetadataValue::signedInteger(-48)),
            "i64 round trip");
    require(exact("frame-count", MetadataValue::unsignedInteger(86400)),
            "u64 round trip");
    require(exact("aspect-ratio", MetadataValue::decimal("239", 2)),
            "decimal round trip");
    require(exact("circled", MetadataValue::boolean(true)), "bool round trip");
    require(exact("shot-at", MetadataValue::timestamp(1'700'000'000'000'000)),
            "timestamp round trip");
    require(
        exact("licence", MetadataValue::uri("https://example.com/licences/7")),
        "URI round trip");
    require(exact("frame-rate", MetadataValue::rational(24000, 1001)),
            "rational round trip");
    require(
        exact("preferred-representation",
              MetadataValue::reference({postproject::ObjectKind::representation,
                                        representation_id})),
        "reference round trip");
    require(!exact("circled", MetadataValue::boolean(false)),
            "exact query rejects other values");
    // Every kind reads back as the value that was written.
    const auto read = [&](const char *property) {
      return production.queryMetadata(editorial, property, 1).value()
          .items.front().value;
    };
    require(read("caption") == MetadataValue::languageString("Am Hafen", "de-DE"),
            "language string reads back");
    require(read("checksum") == MetadataValue::bytes({0xde, 0xad, 0xbe, 0xef}),
            "bytes read back");
    require(read("keywords") ==
                MetadataValue::list({MetadataValue::plainString("interview"),
                                     MetadataValue::plainString("exterior")}),
            "list reads back");
    require(read("slate") ==
                MetadataValue::structure(
                    {{"scene", MetadataValue::plainString("12A")},
                     {"take", MetadataValue::unsignedInteger(3)}}),
            "structure reads back");
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
