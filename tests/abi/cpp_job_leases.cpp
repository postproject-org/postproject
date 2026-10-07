#include <postproject/postproject.hpp>

#include <cstdio>
#include <fstream>
#include <limits>
#include <type_traits>

using namespace std::chrono;
using namespace postproject;

static_assert(!std::is_copy_constructible_v<JobLease>);
static_assert(!std::is_copy_assignable_v<JobLease>);
static_assert(std::is_nothrow_move_constructible_v<JobLease>);
static_assert(std::is_nothrow_move_assignable_v<JobLease>);
static_assert(!std::is_copy_constructible_v<Result<JobLease>>);

int main(int argc, char **argv) {
  if (argc != 2) return 2;
  if (detail::lease_duration(nanoseconds{1}) || detail::lease_duration(seconds{-1}) ||
      detail::lease_duration(hours{25}) || detail::lease_duration(duration<double>{1}) ||
      detail::lease_duration(duration<std::uint64_t>{std::numeric_limits<std::uint64_t>::max()})) return 3;
  if (detail::lease_duration(hours{24}).value() != UINT64_C(86400000000) ||
      detail::lease_duration(nanoseconds{1000}).value() != 1 ||
      detail::lease_duration(duration<int, std::ratio<1, 3>>{3}).value() != 1000000) return 4;
  if (detail::lease_duration(duration<int, std::ratio<1, 3>>{1})) return 5;
  std::remove(argv[1]);
  const std::string media = std::string(argv[1]) + ".mov";
  { std::ofstream file(media); file << "media"; }
  auto production = Production::create(argv[1]);
  if (!production) return 6;
  auto setup = production->beginTransaction();
  if (!setup) return 7;
  auto asset = setup->importMedia(MediaSource(media));
  if (!asset) return 8;
  auto job = setup->requestJob(JobRequest{"example:publish", {}, *asset, RepresentationKind::derived, std::nullopt});
  if (!job || !setup->commit()) return 9;
  const ToolIdentity tool{"worker", std::nullopt, std::nullopt};
  auto edit = production->beginTransaction();
  if (!edit) return 10;
  auto lease = edit->claimJobLease(*job, tool, minutes{1});
  if (!lease || !std::holds_alternative<PendingJobLease>(lease->state().value()) || lease->exportToken()) return 11;
  JobLease moved(std::move(*lease));
  if (lease->state() || !edit->commit()) return 12;
  if (!std::holds_alternative<ActiveJobLease>(moved.state().value()) ||
      moved.jobId().value() != *job || moved.productionId().value() != production->id().value()) return 13;
  auto token = moved.exportToken();
  if (!token) return 14;
  auto imported = production->importJobLease(*token);
  if (!imported || production->importJobLease(std::string(10000, 'x'))) return 15;
  auto renewal = production->beginTransaction();
  if (!renewal || !renewal->renewJobLease(moved, minutes{2}) || !renewal->commit()) return 16;
  auto release = production->beginTransaction();
  if (!release || !release->releaseJobLease(moved) || !release->commit()) return 17;
  if (!std::holds_alternative<ClosedJobLease>(moved.state().value()) || production->importJobLease(*token)) return 18;
  auto late = production->beginTransaction();
  if (!late || !late->failJobLease(*imported, "late worker") || late->commit()) return 19;
  auto publish = production->beginTransaction();
  if (!publish) return 20;
  auto publisher = publish->claimJobLease(*job, tool, minutes{1});
  auto output = publish->addRepresentation(*asset, RepresentationKind::derived, MediaSource(media));
  if (!publisher || !output) return 21;
  ActivitySpec spec;
  spec.kind = "example:publish";
  spec.outputs.push_back(ActivityEdge{*output, std::nullopt});
  spec.tool = tool;
  auto activity = publish->createActivity(spec);
  if (!activity || !publish->completeJobLease(*publisher, *output, *activity) || !publish->commit()) return 22;
  if (!std::holds_alternative<ClosedJobLease>(publisher->state().value())) return 23;
  moved.close();
  if (moved.state()) return 24;
  return 0;
}
