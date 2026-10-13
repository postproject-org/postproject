// Register already-produced outputs using C++17 Result propagation.
#include <postproject/postproject.hpp>
#include <filesystem>
#include <fstream>
#include <iostream>

postproject::Result<void> exercise(const std::filesystem::path &work) {
  const auto render = work / "render.dat";
  const auto thumbnail = work / "thumbnail.dat";
  std::ofstream(render) << "fixture render";
  std::ofstream(thumbnail) << "fixture thumbnail";
  POSTPROJECT_TRY_ASSIGN(auto production,
      postproject::Production::create(work / "outputs.pproj"));
  // [register-outputs]
  postproject::ActivitySpec recipe{};
  recipe.kind = "example:generate";
  recipe.tool = postproject::ToolIdentity{"Fixture generator", "1.0", std::nullopt};
  POSTPROJECT_TRY_ASSIGN(auto edit, production.edit());
  POSTPROJECT_TRY_ASSIGN(const auto registered, edit.registerOutputs(
      {{render}, {thumbnail, std::nullopt, postproject::RepresentationKind::original,
                              "example:thumbnail", std::nullopt}},
      recipe, {{"example:recipe", "width", postproject::MetadataValue::i64(1920)}}));
  POSTPROJECT_TRY_ASSIGN(const auto receipt, edit.commit());
  // No inputs: this operation generated new media. A render supplies input IDs.
  // [/register-outputs]
  if (!receipt.revision || registered.outputs.size() != 2)
    return postproject::Error(postproject::ErrorCode::internal, "missing output receipt");
  for (const auto &output : registered.outputs) {
    POSTPROJECT_TRY_ASSIGN(const auto activities,
        production.activitiesProducing(output.representation_id));
    if (activities.size() != 1 || activities[0].id != registered.activity_id ||
        !activities[0].inputs.empty())
      return postproject::Error(postproject::ErrorCode::internal, "incorrect producer");
  }
  POSTPROJECT_TRY_ASSIGN(const auto parameters,
      production.metadata(postproject::ObjectRef::activity(registered.activity_id)));
  if (parameters.size() != 1)
    return postproject::Error(postproject::ErrorCode::internal, "missing actual parameter");
  POSTPROJECT_TRY_ASSIGN(auto failed, production.edit());
  const auto result = failed.registerOutputs({{render}, {work / "missing.dat"}}, recipe);
  if (result) return postproject::Error(postproject::ErrorCode::internal, "missing file accepted");
  POSTPROJECT_TRY_ASSIGN(const auto unchanged, failed.commit());
  if (unchanged.revision)
    return postproject::Error(postproject::ErrorCode::internal, "partial output prefix");
  return {};
}

int main(int argc, char **argv) {
  if (argc != 2) return 2;
  const auto result = exercise(argv[1]);
  if (!result) { std::cerr << result.error().message() << '\n'; return 1; }
  return 0;
}
