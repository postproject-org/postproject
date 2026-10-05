#include <postproject/postproject.hpp>
#include <stdexcept>
#include <string>
#include <iostream>

// [coherent-reads]
static void exercise(const std::string &path, const std::string &media) {
  auto production = postproject::Production::create(path).value();
  auto empty = production.readSession().value();
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
  auto view = production.readSession().value();
  const auto copied = view.asset(asset).value();
  const auto representations = view.representations(asset, 10).value();
  if (representations.items.size() != 1)
    throw std::runtime_error("representation page");
  const auto representation = view.representation(representations.items[0].id).value();
  const auto resources = view.resources(representation.id, 10).value();
  if (resources.items.size() != 1)
    throw std::runtime_error("resource page");
  const auto locators = view.locators(resources.items[0], 10).value();
  const auto owners = view.representationsUsingResource(resources.items[0], 10).value();
  if (locators.items.size() != 1 || owners.items.size() != 1 ||
      owners.items[0].id != representation.id)
    throw std::runtime_error("locator and resource ownership pages");
  const auto base = view.decisionBase().value();
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
}
// [/coherent-reads]

int main(int argc, char **argv) {
  if (argc != 2) return 2;
  try { exercise(std::string(argv[1]) + "/views.pproj", std::string(argv[1]) + "/rushes/A001.mov"); }
  catch (const std::exception &error) { std::cerr << error.what() << '\n'; return 1; }
}
