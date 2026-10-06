// Wrapper calls compiled one per function, as a host's small adapter calls
// them. Optimizers inline such a call completely and warn more readily than in
// a larger program, so tools/check_cpp_header_warnings.py compiles this file
// with warnings as errors. It is compiled only, never linked or run.
#include <postproject/postproject.hpp>

#include <cstddef>

std::size_t artifact_reason_count(const postproject::Production &production,
                                  const postproject::RepresentationId &representation_id) {
  // GCC 15 reported the evaluation's optional fingerprint values as possibly
  // uninitialized here.
  const auto evaluation = production.evaluateArtifact(representation_id);
  return evaluation.has_value() ? evaluation->reasons.size() : 0;
}
