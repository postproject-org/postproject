#include <postproject/postproject.hpp>
#include <stdexcept>
#include <string>

// [root-pages]
static void root_pages(const std::string &path) {
  auto production = postproject::Production::create(path).value();
  auto transaction = production.beginTransaction().value();
  transaction.addMediaRoot("first", -1).value();
  transaction.addMediaRoot("second").value();
  transaction.commit().value();
  const auto live = production.mediaRoots(1).value();
  if (live.items.size() != 1 || !live.next_cursor)
    throw std::runtime_error("root page continuation");
  auto view = production.readSession().value();
  const auto first = view.mediaRoots(1).value();
  const auto last = view.mediaRoots(1, first.next_cursor).value();
  if (first.items[0].name != "first" || last.items[0].name != "second" || last.next_cursor)
    throw std::runtime_error("coherent root page order");
  if (view.mediaRoots(0).error().code() != postproject::ErrorCode::invalid_argument ||
      view.mediaRoots(1, live.next_cursor).error().code() != postproject::ErrorCode::invalid_argument)
    throw std::runtime_error("root page bounds and cursor scope");
}
// [/root-pages]

int main(int argc, char **argv) {
  if (argc != 2) return 2;
  root_pages(std::string(argv[1]) + "/roots.pproj");
}
