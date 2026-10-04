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
  const auto base = view.decisionBase().value();
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
