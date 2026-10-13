#include <postproject/postproject.hpp>

#include <cstdio>
#include <type_traits>
#include <utility>

// Native ownership must remain enforced by the type system, including when
// an owning handle is carried inside Result rather than stored directly.
template <typename... Owners>
constexpr bool exclusive_owners =
    ((!std::is_copy_constructible_v<Owners> &&
      !std::is_copy_assignable_v<Owners> &&
      std::is_nothrow_move_constructible_v<Owners> &&
      std::is_nothrow_move_assignable_v<Owners>) && ...);
static_assert(exclusive_owners<postproject::Production, postproject::Transaction,
                               postproject::ReadSession,
                               postproject::JobLease,
                               postproject::RevisionWaiter,
                               postproject::CancelToken,
                               postproject::ResolutionOptions>);
static_assert(!std::is_copy_constructible_v<
              postproject::Result<postproject::Production>>);
static_assert(std::is_copy_constructible_v<postproject::MediaSource>);

int main(int argc, char **argv) {
  if (argc != 2)
    return 2;
  std::remove(argv[1]);
  auto production = postproject::Production::create(argv[1], "Lifetime");
  if (!production)
    return 3;
  const auto invalid = production->edit(
      {postproject::OriginIdentity{"", std::nullopt, std::nullopt}, "Invalid"});
  if (invalid || invalid.error().code() != postproject::ErrorCode::invalid_argument)
    return 11;
  auto view = production->readSession();
  if (!view)
    return 12;
  auto retained = view->edit(
      {postproject::OriginIdentity{"Lifetime", "1.0", std::nullopt}, "Add root"});
  if (!retained || !retained->addMediaRoot("archive") || !retained->commit())
    return 13;
  const auto latest = production->latestRevision();
  if (!latest || !*latest || !(*latest)->origin ||
      (*latest)->origin->name != "Lifetime" || (*latest)->message != "Add root")
    return 14;
  for (int terminal = 0; terminal != 3; ++terminal) {
    auto a = production->edit();
    if (!a)
      return 4;
    if (terminal == 2) {
      const postproject::ResourceId absent({1});
      if (!a->confirmLocator(absent, "file:///absent"))
        return 5;
      const auto failed = a->commit();
      if (failed)
        return 6;
    } else if (!(terminal == 0 ? bool(a->commit()) : bool(a->rollback()))) {
      return 7;
    }
    auto b = production->edit();
    if (!b)
      return 8;
    {
      auto moved = std::move(*a);
      // Destruction releases closed A while B is still open.
    }
    const auto rejected = production->edit();
    if (rejected || rejected.error().code() != postproject::ErrorCode::conflict)
      return 9;
    if (!b->rollback())
      return 10;
  }
  return 0;
}
