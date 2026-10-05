#include <postproject/postproject.hpp>

#include <set>
#include <type_traits>
#include <unordered_set>

static_assert(!std::is_convertible_v<postproject::Uuid, postproject::ProductionId>);
static_assert(!std::is_convertible_v<postproject::ProductionId, postproject::Uuid>);
static_assert(!std::is_same_v<postproject::ProductionId, postproject::Uuid>);

int main() {
  auto parsed = postproject::ProductionId::fromString(
      "12345678-1234-5678-9abc-123456789abc");
  if (!parsed.has_value()) return 1;
  const auto id = *parsed;
  const std::set<postproject::ProductionId> ordered{id, id};
  const std::unordered_set<postproject::ProductionId> hashed{id, id};
  if (ordered.size() != 1 || hashed.size() != 1) return 2;
  if (postproject::ProductionId(id.asUuid()) != id) return 3;
  const auto text = id.toString();
  if (!text.has_value() || *text != "12345678-1234-5678-9abc-123456789abc") return 4;
  const auto invalid = postproject::ProductionId::fromString("broken");
  if (invalid.has_value() || invalid.error().code() != postproject::ErrorCode::invalid_argument) return 5;
  const auto embedded = postproject::ProductionId::fromString(std::string_view("valid\0tail", 10));
  if (embedded.has_value() || embedded.error().code() != postproject::ErrorCode::invalid_argument) return 6;
  return 0;
}
