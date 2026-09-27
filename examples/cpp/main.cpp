#include <postproject/postproject.hpp>

#include <exception>
#include <iostream>

int main(int argc, char **argv) {
  if (argc != 3) {
    std::cerr
        << "usage: postproject-cpp-example OUTPUT_PRODUCTION MEDIA_FILE\n";
    return 2;
  }

  try {
    auto production = postproject::Production::create(argv[1], "C++ quickstart").value();
    auto transaction = production.beginTransaction().value();
    const auto asset_id = transaction.importMedia(argv[2], "Quickstart media").value();
    transaction.commit().value();
    std::cout << "representations: "
              << production.representations(asset_id).value().size() << '\n';
  } catch (const postproject::Exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
