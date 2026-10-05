#ifndef POSTPROJECT_POSTPROJECT_HPP
#define POSTPROJECT_POSTPROJECT_HPP

#include <postproject/postproject.h>

#include <array>
#include <atomic>
#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <exception>
#include <functional>
#include <memory>
#include <mutex>
#include <optional>
#include <stdexcept>
#include <string>
#include <string_view>
#include <thread>
#include <type_traits>
#include <utility>
#include <variant>
#include <vector>

namespace postproject {

enum class ErrorCode : std::uint32_t {
  ok = PP_OK,
  invalid_argument = PP_ERROR_INVALID_ARGUMENT,
  not_found = PP_ERROR_NOT_FOUND,
  already_exists = PP_ERROR_ALREADY_EXISTS,
  io = PP_ERROR_IO,
  storage = PP_ERROR_STORAGE,
  migration = PP_ERROR_MIGRATION,
  conflict = PP_ERROR_CONFLICT,
  ambiguous_resolution = PP_ERROR_AMBIGUOUS_RESOLUTION,
  fingerprint = PP_ERROR_FINGERPRINT,
  unsupported = PP_ERROR_UNSUPPORTED,
  cancelled = PP_ERROR_CANCELLED,
  internal = PP_ERROR_INTERNAL,
};

struct TransactionConflict;

// A failure reported by PostProject: a stable code and a diagnostic message.
class Error final {
public:
  Error(ErrorCode code, std::string message,
        std::shared_ptr<const TransactionConflict> transaction_conflict = {})
      : code_(code), message_(std::move(message)),
        transaction_conflict_(std::move(transaction_conflict)) {}

  [[nodiscard]] ErrorCode code() const noexcept { return code_; }
  [[nodiscard]] const std::string &message() const noexcept { return message_; }
  [[nodiscard]] const TransactionConflict *transactionConflict() const noexcept {
    return transaction_conflict_.get();
  }

private:
  ErrorCode code_;
  std::string message_;
  std::shared_ptr<const TransactionConflict> transaction_conflict_;
};

#if defined(__cpp_exceptions) || defined(_CPPUNWIND)
#define POSTPROJECT_HAS_EXCEPTIONS 1
// Thrown by Result::value() when exceptions are enabled.
class Exception final : public std::runtime_error {
public:
  explicit Exception(Error error)
      : std::runtime_error(error.message()), error_(std::move(error)) {}

  [[nodiscard]] ErrorCode code() const noexcept { return error_.code(); }
  [[nodiscard]] const Error &error() const noexcept { return error_; }

private:
  Error error_;
};
#else
#define POSTPROJECT_HAS_EXCEPTIONS 0
#endif

namespace detail {

[[noreturn]] inline void fail(const Error &error) {
#if POSTPROJECT_HAS_EXCEPTIONS
  throw Exception(error);
#else
  std::fprintf(stderr, "postproject: unchecked error %u: %s\n",
               static_cast<unsigned>(error.code()), error.message().c_str());
  std::abort();
#endif
}

template <typename T>
using remove_cvref_t = std::remove_cv_t<std::remove_reference_t<T>>;

} // namespace detail

// Every fallible operation returns a Result: either a value or an Error. The
// members take their names and meanings from C++23 std::expected<T, Error>.
// Check has_value() and read error(), or call value(), which throws Exception
// when exceptions are enabled and otherwise aborts after printing the error.
// A Result is constructed implicitly from an Error.
template <typename T> class [[nodiscard]] Result final {
public:
  using value_type = T;
  using error_type = Error;

  Result(T value) : state_(std::in_place_index<0>, std::move(value)) {}
  Result(Error error) : state_(std::in_place_index<1>, std::move(error)) {}
  template <typename U, typename = std::enable_if_t<
                            std::is_constructible_v<T, U &&> &&
                            !std::is_same_v<std::decay_t<U>, T> &&
                            !std::is_same_v<std::decay_t<U>, Error> &&
                            !std::is_same_v<std::decay_t<U>, Result>>>
  Result(U &&value)
      : state_(std::in_place_index<0>, T(std::forward<U>(value))) {}

  [[nodiscard]] bool has_value() const noexcept { return state_.index() == 0; }
  [[nodiscard]] explicit operator bool() const noexcept { return has_value(); }

  // Precondition: !has_value().
  [[nodiscard]] Error &error() & { return std::get<1>(state_); }
  [[nodiscard]] const Error &error() const & { return std::get<1>(state_); }
  [[nodiscard]] Error &&error() && { return std::get<1>(std::move(state_)); }
  [[nodiscard]] const Error &&error() const && {
    return std::get<1>(std::move(state_));
  }

  T &value() & {
    check();
    return std::get<0>(state_);
  }
  const T &value() const & {
    check();
    return std::get<0>(state_);
  }
  T value() && {
    check();
    return std::get<0>(std::move(state_));
  }

  template <typename U> [[nodiscard]] T value_or(U &&fallback) const & {
    return has_value() ? std::get<0>(state_)
                       : static_cast<T>(std::forward<U>(fallback));
  }
  template <typename U> [[nodiscard]] T value_or(U &&fallback) && {
    return has_value() ? std::get<0>(std::move(state_))
                       : static_cast<T>(std::forward<U>(fallback));
  }

  // Unchecked access. Precondition: has_value().
  [[nodiscard]] T &operator*() & { return std::get<0>(state_); }
  [[nodiscard]] T &&operator*() && { return std::get<0>(std::move(state_)); }
  [[nodiscard]] const T &operator*() const & { return std::get<0>(state_); }
  [[nodiscard]] const T &&operator*() const && {
    return std::get<0>(std::move(state_));
  }
  [[nodiscard]] T *operator->() { return &std::get<0>(state_); }
  [[nodiscard]] const T *operator->() const { return &std::get<0>(state_); }

  // Calls f with the value, which returns a Result<U>; an error passes through.
  template <typename F> auto and_then(F &&f) & {
    return and_then_(*this, std::forward<F>(f));
  }
  template <typename F> auto and_then(F &&f) const & {
    return and_then_(*this, std::forward<F>(f));
  }
  template <typename F> auto and_then(F &&f) && {
    return and_then_(std::move(*this), std::forward<F>(f));
  }
  template <typename F> auto and_then(F &&f) const && {
    return and_then_(std::move(*this), std::forward<F>(f));
  }

  // Calls f with the value and wraps what it returns; an error passes through.
  template <typename F> auto transform(F &&f) & {
    return transform_(*this, std::forward<F>(f));
  }
  template <typename F> auto transform(F &&f) const & {
    return transform_(*this, std::forward<F>(f));
  }
  template <typename F> auto transform(F &&f) && {
    return transform_(std::move(*this), std::forward<F>(f));
  }
  template <typename F> auto transform(F &&f) const && {
    return transform_(std::move(*this), std::forward<F>(f));
  }

  // Calls f with the error, which returns a Result<T>; a value passes through.
  template <typename F> Result or_else(F &&f) & {
    return or_else_(*this, std::forward<F>(f));
  }
  template <typename F> Result or_else(F &&f) const & {
    return or_else_(*this, std::forward<F>(f));
  }
  template <typename F> Result or_else(F &&f) && {
    return or_else_(std::move(*this), std::forward<F>(f));
  }
  template <typename F> Result or_else(F &&f) const && {
    return or_else_(std::move(*this), std::forward<F>(f));
  }

private:
  void check() const {
    if (!has_value()) {
      detail::fail(std::get<1>(state_));
    }
  }

  template <typename Self, typename F>
  static auto and_then_(Self &&self, F &&f) {
    using R = detail::remove_cvref_t<
        std::invoke_result_t<F, decltype(*std::forward<Self>(self))>>;
    static_assert(std::is_same_v<R, Result<typename R::value_type>>,
                  "and_then requires a function that returns a Result");
    if (self.has_value()) {
      return R(std::invoke(std::forward<F>(f), *std::forward<Self>(self)));
    }
    return R(std::forward<Self>(self).error());
  }

  template <typename Self, typename F>
  static auto transform_(Self &&self, F &&f) {
    using U = std::remove_cv_t<
        std::invoke_result_t<F, decltype(*std::forward<Self>(self))>>;
    if (!self.has_value()) {
      return Result<U>(std::forward<Self>(self).error());
    }
    if constexpr (std::is_void_v<U>) {
      std::invoke(std::forward<F>(f), *std::forward<Self>(self));
      return Result<U>();
    } else {
      return Result<U>(
          std::invoke(std::forward<F>(f), *std::forward<Self>(self)));
    }
  }

  template <typename Self, typename F>
  static Result or_else_(Self &&self, F &&f) {
    using R = detail::remove_cvref_t<
        std::invoke_result_t<F, decltype(std::forward<Self>(self).error())>>;
    static_assert(std::is_same_v<R, Result>,
                  "or_else requires a function that returns the same Result");
    if (self.has_value()) {
      return Result(std::forward<Self>(self));
    }
    return std::invoke(std::forward<F>(f), std::forward<Self>(self).error());
  }

  std::variant<T, Error> state_;
};

template <> class [[nodiscard]] Result<void> final {
public:
  using value_type = void;
  using error_type = Error;

  Result() = default;
  Result(Error error) : error_(std::move(error)) {}

  [[nodiscard]] bool has_value() const noexcept { return !error_.has_value(); }
  [[nodiscard]] explicit operator bool() const noexcept { return has_value(); }

  // Precondition: !has_value().
  [[nodiscard]] Error &error() & { return *error_; }
  [[nodiscard]] const Error &error() const & { return *error_; }
  [[nodiscard]] Error &&error() && { return std::move(*error_); }
  [[nodiscard]] const Error &&error() const && { return std::move(*error_); }

  void value() const {
    if (error_.has_value()) {
      detail::fail(*error_);
    }
  }

  // Calls f without arguments, which returns a Result<U>; an error passes
  // through.
  template <typename F> auto and_then(F &&f) const & {
    using R = detail::remove_cvref_t<std::invoke_result_t<F>>;
    static_assert(std::is_same_v<R, Result<typename R::value_type>>,
                  "and_then requires a function that returns a Result");
    return has_value() ? R(std::invoke(std::forward<F>(f))) : R(*error_);
  }
  template <typename F> auto and_then(F &&f) && {
    using R = detail::remove_cvref_t<std::invoke_result_t<F>>;
    static_assert(std::is_same_v<R, Result<typename R::value_type>>,
                  "and_then requires a function that returns a Result");
    return has_value() ? R(std::invoke(std::forward<F>(f)))
                       : R(std::move(*error_));
  }

  // Calls f without arguments and wraps what it returns; an error passes
  // through.
  template <typename F> auto transform(F &&f) const & {
    return transform_(*this, std::forward<F>(f));
  }
  template <typename F> auto transform(F &&f) && {
    return transform_(std::move(*this), std::forward<F>(f));
  }

  // Calls f with the error, which returns a Result<void>; success passes
  // through.
  template <typename F> Result or_else(F &&f) const & {
    return has_value() ? Result()
                       : Result(std::invoke(std::forward<F>(f), *error_));
  }
  template <typename F> Result or_else(F &&f) && {
    return has_value()
               ? Result()
               : Result(std::invoke(std::forward<F>(f), std::move(*error_)));
  }

private:
  template <typename Self, typename F>
  static auto transform_(Self &&self, F &&f) {
    using U = std::remove_cv_t<std::invoke_result_t<F>>;
    if (!self.has_value()) {
      return Result<U>(std::forward<Self>(self).error());
    }
    if constexpr (std::is_void_v<U>) {
      std::invoke(std::forward<F>(f));
      return Result<U>();
    } else {
      return Result<U>(std::invoke(std::forward<F>(f)));
    }
  }

  std::optional<Error> error_;
};

// Propagation helpers for code built with or without exceptions. The
// enclosing function must return a type implicitly constructible from Error,
// such as any Result.
//
// POSTPROJECT_TRY(expression) returns the error of a failed Result.
//
// POSTPROJECT_TRY_ASSIGN(declaration, expression) returns the error of a
// failed Result and otherwise declares a variable holding its value, as in
// POSTPROJECT_TRY_ASSIGN(auto production, Production::open(path)). It expands
// to several statements: use it only directly inside a block, at most once
// per line.
#define POSTPROJECT_DETAIL_CONCAT_(left, right) left##right
#define POSTPROJECT_DETAIL_CONCAT(left, right)                                 \
  POSTPROJECT_DETAIL_CONCAT_(left, right)
#define POSTPROJECT_TRY(expression)                                            \
  do {                                                                         \
    auto pp_try_result_ = (expression);                                        \
    if (!pp_try_result_.has_value()) {                                         \
      return std::move(pp_try_result_).error();                                \
    }                                                                          \
  } while (false)
#define POSTPROJECT_TRY_ASSIGN(declaration, expression)                        \
  auto POSTPROJECT_DETAIL_CONCAT(pp_try_, __LINE__) = (expression);            \
  if (!POSTPROJECT_DETAIL_CONCAT(pp_try_, __LINE__).has_value()) {             \
    return std::move(POSTPROJECT_DETAIL_CONCAT(pp_try_, __LINE__)).error();    \
  }                                                                            \
  declaration = *std::move(POSTPROJECT_DETAIL_CONCAT(pp_try_, __LINE__))

class Uuid final {
public:
  using Bytes = std::array<std::uint8_t, 16>;

  constexpr explicit Uuid(Bytes bytes) noexcept : bytes_(bytes) {}

  [[nodiscard]] constexpr const Bytes &bytes() const noexcept { return bytes_; }

  friend constexpr bool operator==(const Uuid &left,
                                   const Uuid &right) noexcept {
    for (std::size_t index = 0; index < left.bytes_.size(); ++index) {
      if (left.bytes_[index] != right.bytes_[index]) {
        return false;
      }
    }
    return true;
  }

  friend constexpr bool operator!=(const Uuid &left,
                                   const Uuid &right) noexcept {
    return !(left == right);
  }

private:
  Bytes bytes_;
};

// A production identity has standard value semantics. Explicit UUID conversion
// preserves bytes; existence and membership remain operation checks.
class ProductionId final {
public:
  constexpr explicit ProductionId(Uuid value) noexcept : value_(value) {}
  constexpr explicit ProductionId(Uuid::Bytes bytes) noexcept : value_(bytes) {}

  [[nodiscard]] constexpr Uuid asUuid() const noexcept { return value_; }
  [[nodiscard]] constexpr const Uuid::Bytes &bytes() const noexcept {
    return value_.bytes();
  }
  [[nodiscard]] static Result<ProductionId> fromString(std::string_view text);
  [[nodiscard]] Result<std::string> toString() const;

  friend constexpr bool operator==(const ProductionId &left,
                                   const ProductionId &right) noexcept {
    return left.value_ == right.value_;
  }
  friend constexpr bool operator!=(const ProductionId &left,
                                   const ProductionId &right) noexcept {
    return !(left == right);
  }
  friend bool operator<(const ProductionId &left,
                        const ProductionId &right) noexcept {
    return left.bytes() < right.bytes();
  }

private:
  Uuid value_;
};

enum class ObjectKind : std::uint32_t {
  production = PP_OBJECT_PRODUCTION,
  asset = PP_OBJECT_ASSET,
  representation = PP_OBJECT_REPRESENTATION,
  resource = PP_OBJECT_RESOURCE,
  activity = PP_OBJECT_ACTIVITY,
  job = PP_OBJECT_JOB,
};

struct ObjectRef final {
  ObjectKind kind;
  Uuid id;

  friend constexpr bool operator==(const ObjectRef &left,
                                   const ObjectRef &right) noexcept {
    return left.kind == right.kind && left.id == right.id;
  }
};

enum class ConflictKeyKind : std::uint32_t {
  locator_set = PP_CONFLICT_LOCATOR_SET,
  metadata_property = PP_CONFLICT_METADATA_PROPERTY,
  dependency_set = PP_CONFLICT_DEPENDENCY_SET,
  media_root = PP_CONFLICT_MEDIA_ROOT,
  external_identifier = PP_CONFLICT_EXTERNAL_IDENTIFIER,
  resource_fingerprint = PP_CONFLICT_RESOURCE_FINGERPRINT,
  representation_fingerprint = PP_CONFLICT_REPRESENTATION_FINGERPRINT,
};

struct ConflictKey final {
  ConflictKeyKind kind;
  Uuid target_id;
  std::optional<ObjectKind> target_kind;
  std::optional<std::string> namespace_name;
  std::optional<std::string> local_name;
  std::optional<std::string> qualifier;
  std::optional<std::uint16_t> version;
};

struct TransactionConflict final {
  ConflictKey key;
  std::optional<Uuid> base_revision_id;
  std::uint64_t base_revision_sequence;
  Uuid superseding_revision_id;
  std::uint64_t superseding_revision_sequence;
};

struct HostObjectBinding final {
  ProductionId production_id;
  ObjectRef object;

  [[nodiscard]] Result<std::string> toString() const;
  [[nodiscard]] static Result<HostObjectBinding>
  fromString(std::string_view value);

  friend constexpr bool operator==(const HostObjectBinding &left,
                                   const HostObjectBinding &right) noexcept {
    return left.production_id == right.production_id &&
           left.object == right.object;
  }
};

struct ExternalIdentifier final {
  std::string scheme;
  std::string value;
  std::optional<std::string> qualifier;
};

enum class DependencySetStatus : std::uint32_t {
  current = PP_DEPENDENCY_SET_CURRENT,
  needs_extraction = PP_DEPENDENCY_SET_NEEDS_EXTRACTION,
};

struct Dependency final {
  std::optional<Uuid> source_resource_id;
  std::string kind;
  ObjectRef target;
  std::optional<Uuid> resolved_representation_id;
  bool required;
  std::string authored_reference;
};

struct DependencySet final {
  Uuid source_representation_id;
  std::uint64_t recorded_at_revision;
  DependencySetStatus status;
  std::vector<Dependency> dependencies;
};

struct DependencyMatch final {
  ObjectRef target;
  std::uint32_t depth;
};

template <typename T> struct QueryPage final {
  std::vector<T> items;
  std::optional<std::string> next_cursor;
  bool traversal_truncated;
};

// One object reached by a paginated query. Depth is the shortest traversal
// distance for provenance queries and zero for non-traversal queries.
struct ObjectMatch final {
  ObjectRef object;
  std::uint32_t depth;
};

struct FingerprintSnapshot final {
  std::string algorithm;
  std::uint16_t version;
  std::vector<std::uint8_t> value;
  std::optional<std::uint64_t> observed_revision_sequence;
};

struct ActivityEdgeSnapshot final {
  std::uint64_t revision_sequence;
  std::vector<FingerprintSnapshot> fingerprints;
};

struct ActivityEdge final {
  Uuid representation_id;
  std::optional<std::string> role;
  std::optional<ActivityEdgeSnapshot> snapshot = std::nullopt;
};

struct ToolIdentity final {
  std::string name;
  std::optional<std::string> version;
  std::optional<std::string> uri;
};

struct AgentIdentity final {
  std::optional<std::string> name;
  std::optional<ExternalIdentifier> identifier;
};

struct Activity final {
  Uuid id;
  std::string kind;
  std::optional<std::int64_t> started_at_unix_micros;
  std::optional<std::int64_t> finished_at_unix_micros;
  std::optional<ToolIdentity> tool;
  std::optional<AgentIdentity> agent;
  std::vector<ActivityEdge> inputs;
  std::vector<ActivityEdge> outputs;
};

struct ActivitySpec final {
  std::string kind;
  std::optional<std::int64_t> started_at_unix_micros;
  std::optional<std::int64_t> finished_at_unix_micros;
  std::optional<ToolIdentity> tool;
  std::optional<AgentIdentity> agent;
  std::vector<ActivityEdge> inputs;
  std::vector<ActivityEdge> outputs;
};

enum class ArtifactKnowledgeState : std::uint32_t {
  current = PP_ARTIFACT_CURRENT,
  stale = PP_ARTIFACT_STALE,
  indeterminate = PP_ARTIFACT_INDETERMINATE,
  diverged = PP_ARTIFACT_DIVERGED,
};

enum class ArtifactEdgeKind : std::uint32_t {
  input = PP_ARTIFACT_EDGE_INPUT,
  output = PP_ARTIFACT_EDGE_OUTPUT,
};

enum class ArtifactReasonKind : std::uint32_t {
  producing_activity_missing = PP_ARTIFACT_REASON_PRODUCING_ACTIVITY_MISSING,
  producing_activity_ambiguous =
      PP_ARTIFACT_REASON_PRODUCING_ACTIVITY_AMBIGUOUS,
  snapshot_absent = PP_ARTIFACT_REASON_SNAPSHOT_ABSENT,
  fingerprint_evidence_missing =
      PP_ARTIFACT_REASON_FINGERPRINT_EVIDENCE_MISSING,
  fingerprint_changed = PP_ARTIFACT_REASON_FINGERPRINT_CHANGED,
  fingerprint_recomputation_pending =
      PP_ARTIFACT_REASON_FINGERPRINT_RECOMPUTATION_PENDING,
  upstream_not_current = PP_ARTIFACT_REASON_UPSTREAM_NOT_CURRENT,
  traversal_truncated = PP_ARTIFACT_REASON_TRAVERSAL_TRUNCATED,
  dependency_snapshot_absent =
      PP_ARTIFACT_REASON_DEPENDENCY_SNAPSHOT_ABSENT,
  dependency_knowledge_incomplete =
      PP_ARTIFACT_REASON_DEPENDENCY_KNOWLEDGE_INCOMPLETE,
  dependency_path_changed = PP_ARTIFACT_REASON_DEPENDENCY_PATH_CHANGED,
  dependency_fingerprint_changed =
      PP_ARTIFACT_REASON_DEPENDENCY_FINGERPRINT_CHANGED,
  dependency_fingerprint_recomputation_pending =
      PP_ARTIFACT_REASON_DEPENDENCY_FINGERPRINT_RECOMPUTATION_PENDING,
  dependency_fingerprint_evidence_missing =
      PP_ARTIFACT_REASON_DEPENDENCY_FINGERPRINT_EVIDENCE_MISSING,
};

enum class ArtifactDependencyIssue : std::uint32_t {
  needs_extraction = PP_ARTIFACT_DEPENDENCY_NEEDS_EXTRACTION,
  unresolved = PP_ARTIFACT_DEPENDENCY_UNRESOLVED,
  depth_truncated = PP_ARTIFACT_DEPENDENCY_DEPTH_TRUNCATED,
  representations_truncated =
      PP_ARTIFACT_DEPENDENCY_REPRESENTATIONS_TRUNCATED,
};

struct ArtifactDependencyPathSegment final {
  Uuid source_representation_id;
  std::uint32_t dependency_position;
  std::optional<Uuid> source_resource_id;
  std::string kind;
  ObjectRef target;
  std::optional<Uuid> resolved_representation_id;
  std::string authored_reference;
};

enum class ArtifactTraversalLimit : std::uint32_t {
  depth = PP_ARTIFACT_TRAVERSAL_DEPTH,
  representations = PP_ARTIFACT_TRAVERSAL_REPRESENTATIONS,
};

struct ArtifactReason final {
  ArtifactReasonKind kind;
  std::optional<Uuid> activity_id;
  Uuid representation_id;
  std::optional<Uuid> input_representation_id;
  std::optional<ArtifactEdgeKind> edge_kind;
  std::optional<ArtifactKnowledgeState> upstream_state;
  std::optional<ArtifactTraversalLimit> traversal_limit;
  std::optional<std::uint32_t> activity_count;
  std::optional<ArtifactDependencyIssue> dependency_issue;
  std::vector<ArtifactDependencyPathSegment> dependency_path;
  std::optional<std::string> fingerprint_algorithm;
  std::optional<std::uint16_t> fingerprint_version;
  std::optional<std::vector<std::uint8_t>> snapshot_value;
  std::optional<std::vector<std::uint8_t>> current_value;
};

struct ArtifactEvaluation final {
  Uuid representation_id;
  ArtifactKnowledgeState state;
  std::uint32_t visited_representations;
  bool truncated;
  std::vector<ArtifactReason> reasons;
};

enum class ArtifactReproducibilityIssueKind : std::uint32_t {
  producing_activity_missing =
      PP_ARTIFACT_REPRODUCIBILITY_PRODUCING_ACTIVITY_MISSING,
  producing_activity_ambiguous =
      PP_ARTIFACT_REPRODUCIBILITY_PRODUCING_ACTIVITY_AMBIGUOUS,
  tool_identity_missing = PP_ARTIFACT_REPRODUCIBILITY_TOOL_IDENTITY_MISSING,
  parameters_missing = PP_ARTIFACT_REPRODUCIBILITY_PARAMETERS_MISSING,
  input_representation_missing =
      PP_ARTIFACT_REPRODUCIBILITY_INPUT_REPRESENTATION_MISSING,
};

struct ArtifactReproducibilityIssue final {
  ArtifactReproducibilityIssueKind kind;
  std::optional<Uuid> activity_id;
  std::optional<Uuid> representation_id;
  std::optional<std::uint32_t> activity_count;
};

struct ArtifactReproducibility final {
  Uuid representation_id;
  bool reproducible;
  std::optional<Uuid> producing_activity_id;
  std::optional<std::string> activity_kind;
  std::vector<ArtifactReproducibilityIssue> issues;
};

struct OriginIdentity final {
  std::string name;
  std::optional<std::string> version;
  std::optional<std::string> uri;
};

struct RevisionContext final {
  std::optional<OriginIdentity> origin;
  std::optional<std::string> message;
};

struct Revision final {
  Uuid id;
  std::uint64_t sequence;
  Uuid transaction_id;
  std::int64_t committed_at_unix_micros;
  std::optional<OriginIdentity> origin;
  std::optional<std::string> message;
};

struct CommittedRevision final {
  Uuid id;
  std::uint64_t sequence;
};

struct DecisionBase final {
  [[nodiscard]] static Result<DecisionBase> fromToken(std::string_view token);
  [[nodiscard]] Result<std::string> toToken() const;
  ProductionId production_id;
  std::optional<CommittedRevision> revision;
};

struct CommitReceipt final {
  ProductionId production_id;
  std::optional<CommittedRevision> revision;
};

struct AssetImportedEvent final {
  Uuid asset_id;
};

struct RepresentationAddedEvent final {
  Uuid asset_id;
  Uuid representation_id;
};

struct ResourceAddedEvent final {
  Uuid resource_id;
};

struct RepresentationResourceAddedEvent final {
  Uuid representation_id;
  Uuid resource_id;
  std::uint32_t structural_position;
};

struct LocatorAddedEvent final {
  Uuid resource_id;
  Uuid locator_id;
};

struct LocatorRetiredEvent final {
  Uuid resource_id;
  Uuid locator_id;
};

struct MediaRootAddedEvent final {
  Uuid media_root_id;
};

struct MediaRootEnabledChangedEvent final {
  Uuid media_root_id;
  bool enabled;
};

struct MediaRootRemovedEvent final {
  Uuid media_root_id;
};

struct ExternalIdentifierAddedEvent final {
  ObjectRef target;
  ExternalIdentifier identifier;
};

struct ExternalIdentifierRemovedEvent final {
  ObjectRef target;
  ExternalIdentifier identifier;
};

struct MetadataAddedOrReplacedEvent final {
  ObjectRef target;
  std::string vocabulary;
  std::string property;
};

struct MetadataRemovedEvent final {
  ObjectRef target;
  std::string vocabulary;
  std::string property;
};

struct ActivityCreatedEvent final {
  Uuid activity_id;
  std::string kind;
};

struct ActivityInputAddedEvent final {
  Uuid activity_id;
  Uuid representation_id;
  std::optional<std::string> role;
};

struct ActivityOutputAddedEvent final {
  Uuid activity_id;
  Uuid representation_id;
  std::optional<std::string> role;
};

struct ResourceFingerprintObservedEvent final {
  Uuid resource_id;
  std::string algorithm;
  std::uint16_t version;
};

struct RepresentationFingerprintObservedEvent final {
  Uuid representation_id;
  std::string algorithm;
  std::uint16_t version;
};

struct DependencySetRecordedEvent final {
  Uuid representation_id;
};

struct JobRequestedEvent final { Uuid job_id; };
struct JobClaimedEvent final { Uuid job_id; };
struct JobClaimRenewedEvent final { Uuid job_id; };
struct JobClaimReleasedEvent final { Uuid job_id; };
struct JobSucceededEvent final { Uuid job_id; };
struct JobFailedEvent final { Uuid job_id; };
struct JobCancelledEvent final { Uuid job_id; };

using RevisionEventPayload =
    std::variant<AssetImportedEvent, RepresentationAddedEvent,
                 ResourceAddedEvent, RepresentationResourceAddedEvent,
                 LocatorAddedEvent, LocatorRetiredEvent, MediaRootAddedEvent,
                 MediaRootEnabledChangedEvent, MediaRootRemovedEvent,
                 ExternalIdentifierAddedEvent,
                 ExternalIdentifierRemovedEvent,
                 MetadataAddedOrReplacedEvent, MetadataRemovedEvent,
                 ActivityCreatedEvent, ActivityInputAddedEvent,
                 ActivityOutputAddedEvent, ResourceFingerprintObservedEvent,
                 RepresentationFingerprintObservedEvent,
                 DependencySetRecordedEvent, JobRequestedEvent, JobClaimedEvent,
                 JobClaimRenewedEvent, JobClaimReleasedEvent, JobSucceededEvent,
                 JobFailedEvent, JobCancelledEvent>;

struct RevisionEvent final {
  std::uint32_t position;
  RevisionEventPayload payload;
};

// Payload-free event kinds that select revisions for a filtered page.
enum class RevisionEventKind : std::uint32_t {
  asset_imported = PP_REVISION_ASSET_IMPORTED,
  representation_added = PP_REVISION_REPRESENTATION_ADDED,
  resource_added = PP_REVISION_RESOURCE_ADDED,
  representation_resource_added = PP_REVISION_REPRESENTATION_RESOURCE_ADDED,
  locator_added = PP_REVISION_LOCATOR_ADDED,
  locator_retired = PP_REVISION_LOCATOR_RETIRED,
  media_root_added = PP_REVISION_MEDIA_ROOT_ADDED,
  media_root_enabled_changed = PP_REVISION_MEDIA_ROOT_ENABLED_CHANGED,
  media_root_removed = PP_REVISION_MEDIA_ROOT_REMOVED,
  external_identifier_added = PP_REVISION_EXTERNAL_IDENTIFIER_ADDED,
  external_identifier_removed = PP_REVISION_EXTERNAL_IDENTIFIER_REMOVED,
  metadata_added_or_replaced = PP_REVISION_METADATA_ADDED_OR_REPLACED,
  metadata_removed = PP_REVISION_METADATA_REMOVED,
  activity_created = PP_REVISION_ACTIVITY_CREATED,
  activity_input_added = PP_REVISION_ACTIVITY_INPUT_ADDED,
  activity_output_added = PP_REVISION_ACTIVITY_OUTPUT_ADDED,
  resource_fingerprint_observed = PP_REVISION_RESOURCE_FINGERPRINT_OBSERVED,
  representation_fingerprint_observed = PP_REVISION_REPRESENTATION_FINGERPRINT_OBSERVED,
  dependency_set_recorded = PP_REVISION_DEPENDENCY_SET_RECORDED,
  job_requested = PP_REVISION_JOB_REQUESTED,
  job_claimed = PP_REVISION_JOB_CLAIMED,
  job_claim_renewed = PP_REVISION_JOB_CLAIM_RENEWED,
  job_claim_released = PP_REVISION_JOB_CLAIM_RELEASED,
  job_succeeded = PP_REVISION_JOB_SUCCEEDED,
  job_failed = PP_REVISION_JOB_FAILED,
  job_cancelled = PP_REVISION_JOB_CANCELLED,
};

// Matching revisions plus the next cursor: every matching revision with a
// sequence up to `through_sequence` is included.
struct FilteredRevisionPage final {
  std::vector<Revision> revisions;
  std::uint64_t through_sequence;
};

enum class RevisionWaitResult : std::uint32_t {
  revisions = PP_REVISION_WAIT_REVISIONS,
  timed_out = PP_REVISION_WAIT_TIMED_OUT,
  closed = PP_REVISION_WAIT_CLOSED,
  cancelled = PP_REVISION_WAIT_CANCELLED,
};

struct RevisionWait final {
  RevisionWaitResult result;
  // Non-empty only for RevisionWaitResult::revisions.
  std::vector<Revision> revisions;
};

inline constexpr std::chrono::milliseconds max_revision_wait{
    PP_REVISION_WAIT_MAX_TIMEOUT_MILLIS};

struct Asset final {
  Uuid id;
  std::int64_t created_at_unix_micros;
  std::optional<std::string> display_name;
  std::optional<std::string> import_source;
};

struct MediaRoot final {
  Uuid id;
  std::string name;
  std::optional<std::string> label;
  std::optional<std::string> legacy_uri;
  std::int32_t priority;
  bool enabled;
};

enum class VerificationMode : std::uint32_t {
  presence = PP_VERIFY_PRESENCE,
  content = PP_VERIFY_CONTENT,
};

enum class RepresentationKind : std::uint32_t {
  original = PP_REPRESENTATION_ORIGINAL,
  proxy = PP_REPRESENTATION_PROXY,
  optimized = PP_REPRESENTATION_OPTIMIZED,
  derived = PP_REPRESENTATION_DERIVED,
};

enum class ContentStructureKind : std::uint32_t {
  single_resource = PP_CONTENT_SINGLE_RESOURCE,
  image_sequence = PP_CONTENT_IMAGE_SEQUENCE,
  ordered_parts = PP_CONTENT_ORDERED_PARTS,
  package = PP_CONTENT_PACKAGE,
};

enum class LocatorAvailability : std::uint32_t {
  unknown = PP_LOCATOR_UNKNOWN,
  online = PP_LOCATOR_ONLINE,
  offline = PP_LOCATOR_OFFLINE,
};

struct Fingerprint final {
  std::string algorithm;
  std::uint16_t version;
  std::vector<std::uint8_t> value;
};

struct RepresentationMember final {
  Uuid resource_id;
  std::optional<std::string> role;
  bool required;
};

// How the files of an image sequence are named in one directory: prefix, frame
// number zero-padded to at least padding digits, and suffix. A naming belongs
// to a locator; copies of one sequence may name their files differently.
struct SequenceNaming final {
  std::string prefix;
  std::string suffix;
  std::uint8_t padding;

  friend bool operator==(const SequenceNaming &left,
                         const SequenceNaming &right) {
    return left.prefix == right.prefix && left.suffix == right.suffix &&
           left.padding == right.padding;
  }
  friend bool operator!=(const SequenceNaming &left,
                         const SequenceNaming &right) {
    return !(left == right);
  }
};

// Exact current locator evidence used to search for known media. Sequence
// identities include their naming; a directory URI alone is not equivalent.
struct LocatorIdentity final {
  std::string uri;
  std::optional<SequenceNaming> sequence_naming;
};

// What an image sequence is: its frames, rate, and known gaps. Its file names
// belong to each locator.
struct ImageSequenceDescriptor final {
  std::int64_t start;
  std::int64_t end;
  std::uint32_t step;
  std::uint32_t rate_numerator;
  std::uint32_t rate_denominator;
  std::vector<std::int64_t> missing_frames;
};

// The directory and naming become the sequence's first locator.
struct ImageSequenceInput final {
  std::string directory;
  SequenceNaming naming;
  std::int64_t start;
  std::int64_t end;
  std::uint32_t step;
  std::uint32_t rate_numerator;
  std::uint32_t rate_denominator;
  std::vector<std::int64_t> missing_frames;
};

struct FileResourceInput final {
  std::string path;
  std::string role;
  bool required;
};

struct Locator final {
  Uuid id;
  std::string uri;
  LocatorAvailability availability;
  std::optional<std::int64_t> last_seen_unix_micros;
  // Present exactly for a locator of an image-sequence resource.
  std::optional<SequenceNaming> sequence_naming;
};

// A locator returned by a paginated locator query, with its owning resource
// and the logical media root it was confirmed under, when one was recorded.
struct ResourceLocator final {
  Uuid resource_id;
  Locator locator;
  std::optional<std::string> media_root;
};

// One storage match and its owning representation and logical asset.
struct KnownMediaMatch final {
  Uuid asset_id;
  Uuid representation_id;
  Uuid resource_id;
};

struct Resource final {
  Uuid id;
  std::optional<std::uint64_t> file_size;
  std::optional<std::int64_t> modified_at_unix_micros;
  std::vector<Fingerprint> fingerprints;
  std::vector<Locator> locators;
};

struct Representation final {
  Uuid id;
  Uuid asset_id;
  RepresentationKind kind;
  ContentStructureKind structure_kind;
  std::vector<RepresentationMember> members;
  std::optional<ImageSequenceDescriptor> image_sequence;
  std::vector<Fingerprint> fingerprints;
  std::vector<Resource> resources;
};

enum class JobState : std::uint32_t {
  requested = PP_JOB_REQUESTED,
  claimed = PP_JOB_CLAIMED,
  succeeded = PP_JOB_SUCCEEDED,
  failed = PP_JOB_FAILED,
  cancelled = PP_JOB_CANCELLED,
};

struct JobClaim final {
  Uuid id;
  ToolIdentity tool;
  std::optional<AgentIdentity> agent;
  std::int64_t expires_at_unix_micros;
};

struct JobCompletion final {
  Uuid activity_id;
  Uuid representation_id;
};

struct Job final {
  Uuid id;
  std::string kind;
  std::vector<Uuid> inputs;
  Uuid output_asset_id;
  RepresentationKind output_representation_kind;
  std::optional<std::string> target_root;
  JobState state;
  std::optional<JobClaim> claim;
  std::optional<JobCompletion> completion;
  std::optional<std::string> failure_diagnostic;
};

struct JobRequest final {
  std::string kind;
  std::vector<Uuid> inputs;
  Uuid output_asset_id;
  RepresentationKind output_representation_kind;
  std::optional<std::string> target_root;
};

enum class RepresentationAvailability : std::uint32_t {
  online = PP_AVAILABILITY_ONLINE,
  partial = PP_AVAILABILITY_PARTIAL,
  offline = PP_AVAILABILITY_OFFLINE,
  ambiguous = PP_AVAILABILITY_AMBIGUOUS,
  error = PP_AVAILABILITY_ERROR,
};

enum class ResourceResolutionState : std::uint32_t {
  online_at_known_locator = PP_RESOURCE_ONLINE_AT_KNOWN_LOCATOR,
  resolved_exact = PP_RESOURCE_RESOLVED_EXACT,
  resolved_probable = PP_RESOURCE_RESOLVED_PROBABLE,
  offline = PP_RESOURCE_OFFLINE,
  ambiguous = PP_RESOURCE_AMBIGUOUS,
  error = PP_RESOURCE_RESOLUTION_ERROR,
};

enum class AvailabilityIssueKind : std::uint32_t {
  offline_resource = PP_AVAILABILITY_ISSUE_OFFLINE_RESOURCE,
  ambiguous_resource = PP_AVAILABILITY_ISSUE_AMBIGUOUS_RESOURCE,
  resource_error = PP_AVAILABILITY_ISSUE_RESOURCE_ERROR,
  missing_frames = PP_AVAILABILITY_ISSUE_MISSING_FRAMES,
};

enum class EvidenceKind : std::uint32_t {
  known_locator_available = PP_EVIDENCE_KNOWN_LOCATOR_AVAILABLE,
  exact_fingerprint_match = PP_EVIDENCE_EXACT_FINGERPRINT_MATCH,
  full_hash_match = PP_EVIDENCE_FULL_HASH_MATCH,
  partial_fingerprint_match = PP_EVIDENCE_PARTIAL_FINGERPRINT_MATCH,
  file_size_match = PP_EVIDENCE_FILE_SIZE_MATCH,
  file_name_match = PP_EVIDENCE_FILE_NAME_MATCH,
  relative_path_similarity = PP_EVIDENCE_RELATIVE_PATH_SIMILARITY,
  media_root_relation = PP_EVIDENCE_MEDIA_ROOT_RELATION,
  conflicting_candidate = PP_EVIDENCE_CONFLICTING_CANDIDATE,
  discovery_error = PP_EVIDENCE_DISCOVERY_ERROR,
  media_root_unmapped = PP_EVIDENCE_MEDIA_ROOT_UNMAPPED,
  media_root_unavailable = PP_EVIDENCE_MEDIA_ROOT_UNAVAILABLE,
  fingerprint_mismatch = PP_EVIDENCE_FINGERPRINT_MISMATCH,
  fingerprint_not_verified = PP_EVIDENCE_FINGERPRINT_NOT_VERIFIED,
  search_truncated = PP_EVIDENCE_SEARCH_TRUNCATED,
};

enum class ContentVerification : std::uint32_t {
  matches = PP_CONTENT_MATCHES,
  differs = PP_CONTENT_DIFFERS,
  not_comparable = PP_CONTENT_NOT_COMPARABLE,
};

enum class ContentObservationOutcome : std::uint32_t {
  unchanged = PP_OBSERVATION_UNCHANGED,
  changed = PP_OBSERVATION_CHANGED,
  first = PP_OBSERVATION_FIRST,
};

struct Evidence final {
  EvidenceKind kind;
  std::optional<std::string> detail;
};

struct ResolutionCandidate final {
  std::string uri;
  std::uint16_t confidence_basis_points;
  // The logical root the candidate was found under; empty for a candidate
  // found only in an unnamed search directory.
  std::optional<std::string> media_root;
  // The naming the files were found under, for an image-sequence resource.
  // Confirm the candidate with it.
  std::optional<SequenceNaming> sequence_naming;
  std::vector<Evidence> evidence;
};

struct ResourceResolution final {
  Uuid resource_id;
  ResourceResolutionState state;
  std::vector<ResolutionCandidate> candidates;
  std::vector<Evidence> evidence;
};

struct AvailabilityIssue final {
  Uuid resource_id;
  bool required;
  AvailabilityIssueKind kind;
  std::vector<std::int64_t> frames;
};

struct RepresentationResolution final {
  Uuid asset_id;
  Uuid representation_id;
  RepresentationAvailability availability;
  std::vector<ResourceResolution> resources;
  std::vector<AvailabilityIssue> issues;
};

namespace detail {

struct ErrorDeleter final {
  void operator()(pp_error_t *error) const noexcept { pp_error_release(error); }
};

using ErrorHandle = std::unique_ptr<pp_error_t, ErrorDeleter>;

struct AssetSetDeleter final {
  void operator()(pp_asset_set_t *assets) const noexcept {
    pp_asset_set_release(assets);
  }
};

using AssetSetHandle = std::unique_ptr<pp_asset_set_t, AssetSetDeleter>;

struct MediaRootSetDeleter final {
  void operator()(pp_media_root_set_t *roots) const noexcept {
    pp_media_root_set_release(roots);
  }
};

using MediaRootSetHandle =
    std::unique_ptr<pp_media_root_set_t, MediaRootSetDeleter>;

struct ResolutionSetDeleter final {
  void operator()(pp_resolution_set_t *resolutions) const noexcept {
    pp_resolution_set_release(resolutions);
  }
};

using ResolutionSetHandle =
    std::unique_ptr<pp_resolution_set_t, ResolutionSetDeleter>;

struct RepresentationSetDeleter final {
  void operator()(pp_representation_set_t *representations) const noexcept {
    pp_representation_set_release(representations);
  }
};

using RepresentationSetHandle =
    std::unique_ptr<pp_representation_set_t, RepresentationSetDeleter>;

struct ExternalIdentifierSetDeleter final {
  void operator()(pp_external_identifier_set_t *identifiers) const noexcept {
    pp_external_identifier_set_release(identifiers);
  }
};

using ExternalIdentifierSetHandle = std::unique_ptr<
    pp_external_identifier_set_t, ExternalIdentifierSetDeleter>;

struct ObjectRefSetDeleter final {
  void operator()(pp_object_ref_set_t *objects) const noexcept {
    pp_object_ref_set_release(objects);
  }
};

using ObjectRefSetHandle =
    std::unique_ptr<pp_object_ref_set_t, ObjectRefSetDeleter>;

struct ObjectQuerySetDeleter final {
  void operator()(pp_object_query_set_t *objects) const noexcept {
    pp_object_query_set_release(objects);
  }
};

using ObjectQuerySetHandle =
    std::unique_ptr<pp_object_query_set_t, ObjectQuerySetDeleter>;

struct LocatorQuerySetDeleter final {
  void operator()(pp_locator_query_set_t *locators) const noexcept {
    pp_locator_query_set_release(locators);
  }
};

using LocatorQuerySetHandle =
    std::unique_ptr<pp_locator_query_set_t, LocatorQuerySetDeleter>;

struct KnownMediaSetDeleter final {
  void operator()(pp_known_media_set_t *matches) const noexcept {
    pp_known_media_set_release(matches);
  }
};

using KnownMediaSetHandle =
    std::unique_ptr<pp_known_media_set_t, KnownMediaSetDeleter>;

struct DependencySetDeleter final {
  void operator()(pp_dependency_set_t *dependencies) const noexcept {
    pp_dependency_set_release(dependencies);
  }
};

using DependencySetHandle =
    std::unique_ptr<pp_dependency_set_t, DependencySetDeleter>;

struct DependencyQuerySetDeleter final {
  void operator()(pp_dependency_query_set_t *matches) const noexcept {
    pp_dependency_query_set_release(matches);
  }
};

using DependencyQuerySetHandle = std::unique_ptr<
    pp_dependency_query_set_t, DependencyQuerySetDeleter>;

struct ActivitySetDeleter final {
  void operator()(pp_activity_set_t *activities) const noexcept {
    pp_activity_set_release(activities);
  }
};

using ActivitySetHandle =
    std::unique_ptr<pp_activity_set_t, ActivitySetDeleter>;

struct JobSetDeleter final {
  void operator()(pp_job_set_t *jobs) const noexcept {
    pp_job_set_release(jobs);
  }
};

using JobSetHandle = std::unique_ptr<pp_job_set_t, JobSetDeleter>;

struct MetadataSetDeleter final {
  void operator()(pp_metadata_set_t *metadata) const noexcept {
    pp_metadata_set_release(metadata);
  }
};

using MetadataSetHandle =
    std::unique_ptr<pp_metadata_set_t, MetadataSetDeleter>;

struct RegenerationPlanSetDeleter final {
  void operator()(pp_regeneration_plan_set_t *plans) const noexcept {
    pp_regeneration_plan_set_release(plans);
  }
};

using RegenerationPlanSetHandle =
    std::unique_ptr<pp_regeneration_plan_set_t, RegenerationPlanSetDeleter>;

struct ArtifactEvaluationDeleter final {
  void operator()(pp_artifact_evaluation_t *evaluation) const noexcept {
    pp_artifact_evaluation_release(evaluation);
  }
};

using ArtifactEvaluationHandle =
    std::unique_ptr<pp_artifact_evaluation_t, ArtifactEvaluationDeleter>;

struct ArtifactReproducibilityDeleter final {
  void operator()(pp_artifact_reproducibility_t *report) const noexcept {
    pp_artifact_reproducibility_release(report);
  }
};

using ArtifactReproducibilityHandle = std::unique_ptr<
    pp_artifact_reproducibility_t, ArtifactReproducibilityDeleter>;

struct RevisionSetDeleter final {
  void operator()(pp_revision_set_t *revisions) const noexcept {
    pp_revision_set_release(revisions);
  }
};

using RevisionSetHandle =
    std::unique_ptr<pp_revision_set_t, RevisionSetDeleter>;

struct RevisionEventSetDeleter final {
  void operator()(pp_revision_event_set_t *events) const noexcept {
    pp_revision_event_set_release(events);
  }
};

using RevisionEventSetHandle =
    std::unique_ptr<pp_revision_event_set_t, RevisionEventSetDeleter>;

struct StringDeleter final {
  void operator()(char *value) const noexcept { pp_string_release(value); }
};

struct MediaSourceDeleter final {
  void operator()(pp_media_source_t *source) const noexcept {
    pp_media_source_release(source);
  }
};

using MediaSourceHandle = std::unique_ptr<pp_media_source_t, MediaSourceDeleter>;

using StringHandle = std::unique_ptr<char, StringDeleter>;

inline Uuid uuid(const pp_uuid_t &value);
inline std::optional<std::string> optional_string(const char *value);

// raw_error is read by reference so that check(pp_call(..., &error), error) is
// correct whichever argument the compiler evaluates first.
inline Result<void> check(pp_error_code_t status,
                          pp_error_t *const &raw_error) {
  ErrorHandle error(raw_error);
  if (status == PP_OK) {
    return {};
  }
  const char *raw_message = error ? pp_error_message(error.get()) : nullptr;
  std::string message =
      raw_message != nullptr ? raw_message : "PostProject operation failed";
  std::shared_ptr<const TransactionConflict> transaction_conflict;
  pp_transaction_conflict_t native{};
  if (error && pp_error_transaction_conflict(error.get(), &native) != 0) {
    const auto kind = static_cast<ConflictKeyKind>(native.kind);
    const bool fingerprint =
        kind == ConflictKeyKind::resource_fingerprint ||
        kind == ConflictKeyKind::representation_fingerprint;
    std::optional<ObjectKind> target_kind;
    if (native.target.kind != 0) {
      target_kind = static_cast<ObjectKind>(native.target.kind);
    }
    transaction_conflict = std::make_shared<TransactionConflict>(
        TransactionConflict{
            {kind, uuid(native.target.id), target_kind,
             optional_string(native.namespace_name),
             optional_string(native.local_name), optional_string(native.qualifier),
             fingerprint ? std::optional<std::uint16_t>(native.version)
                         : std::nullopt},
            native.has_base_revision ? std::optional<Uuid>(uuid(native.base_revision_id))
                                     : std::nullopt,
            native.base_revision_sequence,
            uuid(native.superseding_revision_id),
            native.superseding_revision_sequence});
  }
  return Error(static_cast<ErrorCode>(status), std::move(message),
               std::move(transaction_conflict));
}

inline Result<std::string> checked_string(std::string_view value,
                                          std::string_view label) {
  if (value.find('\0') != std::string_view::npos) {
    return Error(ErrorCode::invalid_argument,
                 std::string(label) + " must not contain an embedded NUL");
  }
  return std::string(value);
}

inline Uuid uuid(const pp_uuid_t &value) {
  Uuid::Bytes bytes{};
  for (std::size_t index = 0; index < bytes.size(); ++index) {
    bytes[index] = value.bytes[index];
  }
  return Uuid(bytes);
}

inline pp_uuid_t native_uuid(const Uuid &value) {
  pp_uuid_t native{};
  for (std::size_t index = 0; index < value.bytes().size(); ++index) {
    native.bytes[index] = value.bytes()[index];
  }
  return native;
}

inline ProductionId production_id(const pp_production_id_t &value) {
  Uuid::Bytes bytes{};
  std::copy(std::begin(value.bytes), std::end(value.bytes), bytes.begin());
  return ProductionId(bytes);
}

inline pp_production_id_t native_production_id(const ProductionId &value) {
  pp_production_id_t native{};
  std::copy(value.bytes().begin(), value.bytes().end(), std::begin(native.bytes));
  return native;
}

inline ObjectRef object_ref(const pp_object_ref_t &value) {
  return {static_cast<ObjectKind>(value.kind), uuid(value.id)};
}

inline pp_object_ref_t native_object_ref(const ObjectRef &value) {
  return {static_cast<pp_object_kind_t>(value.kind), native_uuid(value.id)};
}

inline std::optional<std::string> optional_string(const char *value) {
  return value != nullptr
             ? std::optional<std::string>(std::string(value))
             : std::nullopt;
}

inline std::optional<SequenceNaming>
optional_naming(std::uint8_t present, const pp_sequence_naming_t &naming) {
  if (present == 0) {
    return std::nullopt;
  }
  return SequenceNaming{
      naming.prefix != nullptr ? std::string(naming.prefix) : std::string(),
      naming.suffix != nullptr ? std::string(naming.suffix) : std::string(),
      naming.padding};
}

// A checked copy of a naming whose view stays valid while this value lives.
struct NativeNaming final {
public:
  static Result<NativeNaming> make(const std::optional<SequenceNaming> &naming) {
    NativeNaming native;
    if (naming.has_value()) {
      POSTPROJECT_TRY_ASSIGN(native.prefix_,
                             checked_string(naming->prefix, "naming prefix"));
      POSTPROJECT_TRY_ASSIGN(native.suffix_,
                             checked_string(naming->suffix, "naming suffix"));
      native.padding_ = naming->padding;
      native.present_ = true;
    }
    return native;
  }

  // Null when no naming was given.
  [[nodiscard]] const pp_sequence_naming_t *get() {
    if (!present_) {
      return nullptr;
    }
    view_ = {prefix_.c_str(), suffix_.c_str(), padding_};
    return &view_;
  }

private:
  std::string prefix_;
  std::string suffix_;
  std::uint8_t padding_ = 0;
  bool present_ = false;
  pp_sequence_naming_t view_{};
};

inline Result<std::optional<std::vector<std::uint8_t>>>
optional_bytes(std::uint8_t present, const std::uint8_t *value,
               std::uint64_t length) {
  if (present == 0) {
    return std::nullopt;
  }
  if (length != 0 && value == nullptr) {
    return Error(ErrorCode::internal, "artifact evidence bytes are null");
  }
  if (length == 0) {
    return std::vector<std::uint8_t>();
  }
  return std::vector<std::uint8_t>(value, value + length);
}

inline Result<std::optional<std::string>>
checked_optional_string(const std::optional<std::string> &value,
                        std::string_view label) {
  if (!value.has_value()) {
    return std::optional<std::string>();
  }
  POSTPROJECT_TRY_ASSIGN(std::string checked, checked_string(*value, label));
  return std::optional<std::string>(std::move(checked));
}

inline Result<std::optional<std::string>>
checked_cursor(const std::optional<std::string_view> &cursor) {
  if (!cursor.has_value()) {
    return std::optional<std::string>();
  }
  POSTPROJECT_TRY_ASSIGN(std::string checked,
                         checked_string(*cursor, "query cursor"));
  return std::optional<std::string>(std::move(checked));
}

inline const char *
optional_c_str(const std::optional<std::string> &value) noexcept {
  return value.has_value() ? value->c_str() : nullptr;
}

inline Result<Asset> asset(const pp_asset_set_t *assets, std::uint64_t index) {
  pp_uuid_t id{};
  std::int64_t created_at_unix_micros = 0;
  const char *display_name = nullptr;
  const char *import_source = nullptr;
  pp_error_t *error = nullptr;
  const pp_error_code_t status =
      pp_asset_set_get(assets, index, &id, &created_at_unix_micros,
                       &display_name, &import_source, &error);
  POSTPROJECT_TRY(check(status, error));
  return Asset{uuid(id), created_at_unix_micros, optional_string(display_name),
               optional_string(import_source)};
}

inline Result<Fingerprint>
representation_fingerprint(const pp_representation_set_t *representations,
                           std::uint64_t representation_index,
                           std::uint64_t fingerprint_index) {
  const char *algorithm = nullptr;
  std::uint16_t version = 0;
  const std::uint8_t *value = nullptr;
  std::uint64_t value_length = 0;
  pp_error_t *error = nullptr;
  const pp_error_code_t status = pp_representation_set_get_fingerprint(
      representations, representation_index, fingerprint_index, &algorithm,
      &version, &value, &value_length, &error);
  POSTPROJECT_TRY(check(status, error));
  std::vector<std::uint8_t> copied_value;
  if (value != nullptr) {
    copied_value.assign(value, value + value_length);
  }
  return Fingerprint{algorithm != nullptr ? std::string(algorithm)
                                          : std::string(),
                     version, std::move(copied_value)};
}

inline Result<Fingerprint>
resource_fingerprint(const pp_representation_set_t *representations,
                     std::uint64_t representation_index,
                     std::uint64_t resource_index,
                     std::uint64_t fingerprint_index) {
  const char *algorithm = nullptr;
  std::uint16_t version = 0;
  const std::uint8_t *value = nullptr;
  std::uint64_t value_length = 0;
  pp_error_t *error = nullptr;
  const pp_error_code_t status =
      pp_representation_set_get_resource_fingerprint(
          representations, representation_index, resource_index,
          fingerprint_index, &algorithm, &version, &value, &value_length,
          &error);
  POSTPROJECT_TRY(check(status, error));
  std::vector<std::uint8_t> copied_value;
  if (value != nullptr) {
    copied_value.assign(value, value + value_length);
  }
  return Fingerprint{algorithm != nullptr ? std::string(algorithm)
                                          : std::string(),
                     version, std::move(copied_value)};
}

inline Result<Representation>
representation(const pp_representation_set_t *representations,
               std::uint64_t index) {
  pp_uuid_t id{};
  pp_uuid_t asset_id{};
  pp_representation_kind_t kind = 0;
  pp_content_structure_kind_t structure_kind = 0;
  std::uint64_t member_count = 0;
  std::uint64_t resource_count = 0;
  std::uint64_t fingerprint_count = 0;
  pp_error_t *error = nullptr;
  pp_error_code_t status = pp_representation_set_get(
      representations, index, &id, &asset_id, &kind, &structure_kind,
      &member_count, &resource_count, &fingerprint_count, &error);
  POSTPROJECT_TRY(check(status, error));

  std::vector<RepresentationMember> members;
  members.reserve(static_cast<std::size_t>(member_count));
  for (std::uint64_t member_index = 0; member_index < member_count;
       ++member_index) {
    pp_uuid_t resource_id{};
    const char *role = nullptr;
    std::uint8_t required = 0;
    error = nullptr;
    status = pp_representation_set_get_member(
        representations, index, member_index, &resource_id, &role, &required,
        &error);
    POSTPROJECT_TRY(check(status, error));
    members.push_back({uuid(resource_id), optional_string(role), required != 0});
  }

  std::optional<ImageSequenceDescriptor> image_sequence;
  if (structure_kind == PP_CONTENT_IMAGE_SEQUENCE) {
    std::int64_t start = 0;
    std::int64_t end = 0;
    std::uint32_t step = 0;
    std::uint32_t rate_numerator = 0;
    std::uint32_t rate_denominator = 0;
    std::uint64_t missing_count = 0;
    error = nullptr;
    status = pp_representation_set_get_sequence(
        representations, index, &start, &end, &step, &rate_numerator,
        &rate_denominator, &missing_count, &error);
    POSTPROJECT_TRY(check(status, error));
    std::vector<std::int64_t> missing_frames;
    missing_frames.reserve(static_cast<std::size_t>(missing_count));
    for (std::uint64_t frame_index = 0; frame_index < missing_count;
         ++frame_index) {
      std::int64_t frame = 0;
      error = nullptr;
      status = pp_representation_set_get_sequence_missing_frame(
          representations, index, frame_index, &frame, &error);
      POSTPROJECT_TRY(check(status, error));
      missing_frames.push_back(frame);
    }
    image_sequence = ImageSequenceDescriptor{
        start,
        end,
        step,
        rate_numerator,
        rate_denominator,
        std::move(missing_frames)};
  }

  std::vector<Fingerprint> fingerprints;
  fingerprints.reserve(static_cast<std::size_t>(fingerprint_count));
  for (std::uint64_t fingerprint_index = 0;
       fingerprint_index < fingerprint_count; ++fingerprint_index) {
    POSTPROJECT_TRY_ASSIGN(
        auto item_1,
        representation_fingerprint(representations, index, fingerprint_index));
    fingerprints.push_back(std::move(item_1));
  }

  std::vector<Resource> resources;
  resources.reserve(static_cast<std::size_t>(resource_count));
  for (std::uint64_t resource_index = 0; resource_index < resource_count;
       ++resource_index) {
    pp_uuid_t resource_id{};
    std::uint8_t has_file_facts = 0;
    std::uint64_t file_size = 0;
    std::uint8_t has_modified_at = 0;
    std::int64_t modified_at = 0;
    std::uint64_t locator_count = 0;
    std::uint64_t resource_fingerprint_count = 0;
    error = nullptr;
    status = pp_representation_set_get_resource(
        representations, index, resource_index, &resource_id, &has_file_facts,
        &file_size, &has_modified_at, &modified_at, &locator_count,
        &resource_fingerprint_count, &error);
    POSTPROJECT_TRY(check(status, error));

    std::vector<Fingerprint> resource_fingerprints;
    resource_fingerprints.reserve(
        static_cast<std::size_t>(resource_fingerprint_count));
    for (std::uint64_t fingerprint_index = 0;
         fingerprint_index < resource_fingerprint_count; ++fingerprint_index) {
      POSTPROJECT_TRY_ASSIGN(
          auto item_2, resource_fingerprint(representations, index,
                                            resource_index, fingerprint_index));
      resource_fingerprints.push_back(std::move(item_2));
    }

    std::vector<Locator> locators;
    locators.reserve(static_cast<std::size_t>(locator_count));
    for (std::uint64_t locator_index = 0; locator_index < locator_count;
         ++locator_index) {
      pp_uuid_t locator_id{};
      const char *uri = nullptr;
      pp_locator_availability_t availability = 0;
      std::uint8_t has_last_seen = 0;
      std::int64_t last_seen = 0;
      std::uint8_t has_naming = 0;
      pp_sequence_naming_t naming{};
      error = nullptr;
      status = pp_representation_set_get_locator(
          representations, index, resource_index, locator_index, &locator_id,
          &uri, &availability, &has_last_seen, &last_seen, &has_naming,
          &naming, &error);
      POSTPROJECT_TRY(check(status, error));
      locators.push_back(
          {uuid(locator_id), uri != nullptr ? std::string(uri) : std::string(),
           static_cast<LocatorAvailability>(availability),
           has_last_seen != 0
               ? std::optional<std::int64_t>(last_seen)
               : std::nullopt,
           optional_naming(has_naming, naming)});
    }
    resources.push_back(
        {uuid(resource_id),
         has_file_facts != 0 ? std::optional<std::uint64_t>(file_size)
                             : std::nullopt,
         has_modified_at != 0 ? std::optional<std::int64_t>(modified_at)
                              : std::nullopt,
         std::move(resource_fingerprints), std::move(locators)});
  }

  return Representation{uuid(id),
                        uuid(asset_id),
                        static_cast<RepresentationKind>(kind),
                        static_cast<ContentStructureKind>(structure_kind),
                        std::move(members),
                        std::move(image_sequence),
                        std::move(fingerprints),
                        std::move(resources)};
}

inline Result<std::optional<ActivityEdgeSnapshot>>
activity_edge_snapshot(const pp_activity_set_t *activities,
                       std::uint64_t activity_index, std::uint64_t edge_index,
                       bool output) {
  std::uint8_t has_snapshot = 0;
  std::uint64_t revision_sequence = 0;
  std::uint64_t fingerprint_count = 0;
  pp_error_t *error = nullptr;
  pp_error_code_t status =
      output ? pp_activity_set_get_output_snapshot(
                   activities, activity_index, edge_index, &has_snapshot,
                   &revision_sequence, &fingerprint_count, &error)
             : pp_activity_set_get_input_snapshot(
                   activities, activity_index, edge_index, &has_snapshot,
                   &revision_sequence, &fingerprint_count, &error);
  POSTPROJECT_TRY(check(status, error));
  if (has_snapshot == 0) {
    return std::nullopt;
  }

  std::vector<FingerprintSnapshot> fingerprints;
  fingerprints.reserve(static_cast<std::size_t>(fingerprint_count));
  for (std::uint64_t index = 0; index < fingerprint_count; ++index) {
    const char *algorithm = nullptr;
    std::uint16_t version = 0;
    const std::uint8_t *value = nullptr;
    std::uint64_t value_length = 0;
    std::uint8_t has_observed_revision = 0;
    std::uint64_t observed_revision_sequence = 0;
    error = nullptr;
    status = output
                 ? pp_activity_set_get_output_snapshot_fingerprint(
                       activities, activity_index, edge_index, index,
                       &algorithm, &version, &value, &value_length,
                       &has_observed_revision, &observed_revision_sequence,
                       &error)
                 : pp_activity_set_get_input_snapshot_fingerprint(
                       activities, activity_index, edge_index, index,
                       &algorithm, &version, &value, &value_length,
                       &has_observed_revision, &observed_revision_sequence,
                       &error);
    POSTPROJECT_TRY(check(status, error));
    fingerprints.push_back(
        {algorithm != nullptr ? std::string(algorithm) : std::string(), version,
         std::vector<std::uint8_t>(value, value + value_length),
         has_observed_revision != 0
             ? std::optional<std::uint64_t>(observed_revision_sequence)
             : std::nullopt});
  }
  return ActivityEdgeSnapshot{revision_sequence, std::move(fingerprints)};
}

inline Result<Activity> activity(const pp_activity_set_t *activities,
                                 std::uint64_t index) {
  pp_uuid_t id{};
  const char *kind = nullptr;
  std::uint8_t has_started_at = 0;
  std::int64_t started_at = 0;
  std::uint8_t has_finished_at = 0;
  std::int64_t finished_at = 0;
  std::uint64_t input_count = 0;
  std::uint64_t output_count = 0;
  pp_error_t *error = nullptr;
  pp_error_code_t status = pp_activity_set_get(
      activities, index, &id, &kind, &has_started_at, &started_at,
      &has_finished_at, &finished_at, &input_count, &output_count, &error);
  POSTPROJECT_TRY(check(status, error));

  const char *tool_name = nullptr;
  const char *tool_version = nullptr;
  const char *tool_uri = nullptr;
  error = nullptr;
  status = pp_activity_set_get_tool(activities, index, &tool_name,
                                    &tool_version, &tool_uri, &error);
  POSTPROJECT_TRY(check(status, error));
  std::optional<ToolIdentity> tool;
  if (tool_name != nullptr) {
    tool = ToolIdentity{std::string(tool_name), optional_string(tool_version),
                        optional_string(tool_uri)};
  }

  const char *agent_name = nullptr;
  const char *agent_scheme = nullptr;
  const char *agent_value = nullptr;
  const char *agent_qualifier = nullptr;
  error = nullptr;
  status = pp_activity_set_get_agent(
      activities, index, &agent_name, &agent_scheme, &agent_value,
      &agent_qualifier, &error);
  POSTPROJECT_TRY(check(status, error));
  std::optional<AgentIdentity> agent;
  if (agent_name != nullptr || agent_scheme != nullptr) {
    std::optional<ExternalIdentifier> identifier;
    if (agent_scheme != nullptr && agent_value != nullptr) {
      identifier = ExternalIdentifier{std::string(agent_scheme),
                                      std::string(agent_value),
                                      optional_string(agent_qualifier)};
    }
    agent = AgentIdentity{optional_string(agent_name), std::move(identifier)};
  }

  std::vector<ActivityEdge> inputs;
  inputs.reserve(static_cast<std::size_t>(input_count));
  for (std::uint64_t edge_index = 0; edge_index < input_count; ++edge_index) {
    pp_uuid_t representation_id{};
    const char *role = nullptr;
    error = nullptr;
    status = pp_activity_set_get_input(activities, index, edge_index,
                                       &representation_id, &role, &error);
    POSTPROJECT_TRY(check(status, error));
    POSTPROJECT_TRY_ASSIGN(
        std::optional<ActivityEdgeSnapshot> snapshot,
        activity_edge_snapshot(activities, index, edge_index, false));
    inputs.push_back(
        {uuid(representation_id), optional_string(role), std::move(snapshot)});
  }
  std::vector<ActivityEdge> outputs;
  outputs.reserve(static_cast<std::size_t>(output_count));
  for (std::uint64_t edge_index = 0; edge_index < output_count; ++edge_index) {
    pp_uuid_t representation_id{};
    const char *role = nullptr;
    error = nullptr;
    status = pp_activity_set_get_output(activities, index, edge_index,
                                        &representation_id, &role, &error);
    POSTPROJECT_TRY(check(status, error));
    POSTPROJECT_TRY_ASSIGN(
        std::optional<ActivityEdgeSnapshot> snapshot,
        activity_edge_snapshot(activities, index, edge_index, true));
    outputs.push_back(
        {uuid(representation_id), optional_string(role), std::move(snapshot)});
  }

  return Activity{uuid(id),
                  kind != nullptr ? std::string(kind) : std::string(),
                  has_started_at != 0 ? std::optional<std::int64_t>(started_at)
                                      : std::nullopt,
                  has_finished_at != 0
                      ? std::optional<std::int64_t>(finished_at)
                      : std::nullopt,
                  std::move(tool),
                  std::move(agent),
                  std::move(inputs),
                  std::move(outputs)};
}

inline Result<Job> job(const pp_job_set_t *jobs, std::uint64_t index) {
  pp_job_t native{};
  pp_error_t *error = nullptr;
  const pp_error_code_t status =
      pp_job_set_get(jobs, index, &native, &error);
  POSTPROJECT_TRY(check(status, error));

  std::vector<Uuid> inputs;
  inputs.reserve(static_cast<std::size_t>(native.input_count));
  for (std::uint64_t input_index = 0; input_index < native.input_count;
       ++input_index) {
    pp_uuid_t input{};
    error = nullptr;
    const pp_error_code_t input_status = pp_job_set_get_input(
        jobs, index, input_index, &input, &error);
    POSTPROJECT_TRY(check(input_status, error));
    inputs.push_back(uuid(input));
  }

  const JobState state = static_cast<JobState>(native.state);
  std::optional<JobClaim> claim;
  if (state == JobState::claimed) {
    std::optional<ExternalIdentifier> identifier;
    if (native.claim_agent_identifier_scheme != nullptr &&
        native.claim_agent_identifier_value != nullptr) {
      identifier = ExternalIdentifier{
          std::string(native.claim_agent_identifier_scheme),
          std::string(native.claim_agent_identifier_value),
          optional_string(native.claim_agent_identifier_qualifier)};
    }
    std::optional<AgentIdentity> agent;
    if (native.claim_agent_name != nullptr || identifier.has_value()) {
      agent = AgentIdentity{optional_string(native.claim_agent_name),
                            std::move(identifier)};
    }
    claim = JobClaim{
        uuid(native.claim_id),
        ToolIdentity{native.claim_tool_name != nullptr
                         ? std::string(native.claim_tool_name)
                         : std::string(),
                     optional_string(native.claim_tool_version),
                     optional_string(native.claim_tool_uri)},
        std::move(agent), native.claim_expires_at_unix_micros};
  }
  std::optional<JobCompletion> completion;
  if (state == JobState::succeeded) {
    completion = JobCompletion{uuid(native.completion_activity_id),
                               uuid(native.completion_representation_id)};
  }
  return Job{uuid(native.id),
             native.kind != nullptr ? std::string(native.kind) : std::string(),
             std::move(inputs),
             uuid(native.output_asset_id),
             static_cast<RepresentationKind>(native.output_representation_kind),
             optional_string(native.target_root),
             state,
             std::move(claim),
             std::move(completion),
             optional_string(native.failure_diagnostic)};
}

inline Result<QueryPage<DependencyMatch>>
dependency_query_page(DependencyQuerySetHandle matches) {
  std::vector<DependencyMatch> items;
  const std::uint64_t count = pp_dependency_query_set_count(matches.get());
  items.reserve(static_cast<std::size_t>(count));
  for (std::uint64_t index = 0; index < count; ++index) {
    pp_dependency_match_t native{};
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_dependency_query_set_get(
        matches.get(), index, &native, &error);
    POSTPROJECT_TRY(check(status, error));
    items.push_back({object_ref(native.target), native.depth});
  }
  const char *cursor = pp_dependency_query_set_next_cursor(matches.get());
  return QueryPage<DependencyMatch>{
      std::move(items), optional_string(cursor),
      pp_dependency_query_set_traversal_truncated(matches.get()) != 0};
}

inline Result<QueryPage<ObjectMatch>>
object_query_page(ObjectQuerySetHandle objects) {
  std::vector<ObjectMatch> items;
  const std::uint64_t count = pp_object_query_set_count(objects.get());
  items.reserve(static_cast<std::size_t>(count));
  for (std::uint64_t index = 0; index < count; ++index) {
    pp_object_ref_t object{};
    std::uint32_t depth = 0;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_object_query_set_get(
        objects.get(), index, &object, &depth, &error);
    POSTPROJECT_TRY(check(status, error));
    items.push_back({object_ref(object), depth});
  }
  const char *cursor = pp_object_query_set_next_cursor(objects.get());
  return QueryPage<ObjectMatch>{
      std::move(items), optional_string(cursor),
      pp_object_query_set_traversal_truncated(objects.get()) != 0};
}

inline QueryPage<ObjectRef> object_ref_page(QueryPage<ObjectMatch> page) {
  std::vector<ObjectRef> items;
  items.reserve(page.items.size());
  for (const ObjectMatch &match : page.items) {
    items.push_back(match.object);
  }
  return {std::move(items), std::move(page.next_cursor),
          page.traversal_truncated};
}

inline Result<QueryPage<Uuid>> object_id_page(QueryPage<ObjectMatch> page,
                                              ObjectKind kind) {
  std::vector<Uuid> items;
  items.reserve(page.items.size());
  for (const ObjectMatch &match : page.items) {
    if (match.object.kind != kind) {
      return Error(ErrorCode::internal,
                   "object query returned an unexpected object kind");
    }
    items.push_back(match.object.id);
  }
  return QueryPage<Uuid>{std::move(items), std::move(page.next_cursor),
                         page.traversal_truncated};
}

inline Result<QueryPage<ResourceLocator>>
locator_page(LocatorQuerySetHandle locators) {

  std::vector<ResourceLocator> items;
  const std::uint64_t count = pp_locator_query_set_count(locators.get());
  items.reserve(static_cast<std::size_t>(count));
  for (std::uint64_t index = 0; index < count; ++index) {
    pp_uuid_t id{};
    pp_uuid_t owner_id{};
    const char *uri = nullptr;
    pp_locator_availability_t availability = 0;
    std::uint8_t has_last_seen = 0;
    std::int64_t last_seen = 0;
    const char *media_root = nullptr;
    std::uint8_t has_naming = 0;
    pp_sequence_naming_t naming{};
    pp_error_t *item_error = nullptr;
    const pp_error_code_t item_status = pp_locator_query_set_get(
        locators.get(), index, &id, &owner_id, &uri, &availability,
        &has_last_seen, &last_seen, &media_root, &has_naming, &naming,
        &item_error);
    POSTPROJECT_TRY(check(item_status, item_error));
    if (uri == nullptr) {
      return Error(ErrorCode::internal, "locator has no URI");
    }
    items.push_back(
        {uuid(owner_id),
         Locator{uuid(id), std::string(uri),
                 static_cast<LocatorAvailability>(availability),
                 has_last_seen != 0 ? std::optional<std::int64_t>(last_seen)
                                    : std::nullopt,
                 optional_naming(has_naming, naming)},
         optional_string(media_root)});
  }
  const char *next_cursor = pp_locator_query_set_next_cursor(locators.get());
  return QueryPage<ResourceLocator>{
      std::move(items), optional_string(next_cursor), false};
}

inline Result<QueryPage<Representation>>
representation_page(RepresentationSetHandle representations) {
  std::vector<Representation> items;
  const std::uint64_t count =
      pp_representation_set_count(representations.get());
  items.reserve(static_cast<std::size_t>(count));
  for (std::uint64_t index = 0; index < count; ++index) {
    POSTPROJECT_TRY_ASSIGN(auto item_1,
                           representation(representations.get(), index));
    items.push_back(std::move(item_1));
  }
  const char *cursor = pp_representation_set_next_cursor(representations.get());
  return QueryPage<Representation>{std::move(items), optional_string(cursor),
                                   false};
}

inline Result<QueryPage<KnownMediaMatch>>
known_media_page(KnownMediaSetHandle matches) {
  std::vector<KnownMediaMatch> items;
  const std::uint64_t count = pp_known_media_set_count(matches.get());
  items.reserve(static_cast<std::size_t>(count));
  for (std::uint64_t index = 0; index < count; ++index) {
    pp_uuid_t asset_id{};
    pp_uuid_t representation_id{};
    pp_uuid_t resource_id{};
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_known_media_set_get(
        matches.get(), index, &asset_id, &representation_id, &resource_id,
        &error);
    POSTPROJECT_TRY(check(status, error));
    items.push_back(
        {uuid(asset_id), uuid(representation_id), uuid(resource_id)});
  }
  const char *cursor = pp_known_media_set_next_cursor(matches.get());
  return QueryPage<KnownMediaMatch>{std::move(items), optional_string(cursor),
                                    false};
}

inline Result<QueryPage<Activity>> activity_page(ActivitySetHandle activities) {
  std::vector<Activity> items;
  const std::uint64_t count = pp_activity_set_count(activities.get());
  items.reserve(static_cast<std::size_t>(count));
  for (std::uint64_t index = 0; index < count; ++index) {
    POSTPROJECT_TRY_ASSIGN(auto item_3, activity(activities.get(), index));
    items.push_back(std::move(item_3));
  }
  const char *cursor = pp_activity_set_next_cursor(activities.get());
  return QueryPage<Activity>{std::move(items), optional_string(cursor), false};
}

inline Result<Revision> revision(const pp_revision_set_t *revisions,
                                 std::uint64_t index) {
  pp_uuid_t id{};
  std::uint64_t sequence = 0;
  pp_uuid_t transaction_id{};
  std::int64_t committed_at = 0;
  const char *origin_name = nullptr;
  const char *origin_version = nullptr;
  const char *origin_uri = nullptr;
  const char *message = nullptr;
  pp_error_t *error = nullptr;
  const pp_error_code_t status = pp_revision_set_get(
      revisions, index, &id, &sequence, &transaction_id, &committed_at,
      &origin_name, &origin_version, &origin_uri, &message, &error);
  POSTPROJECT_TRY(check(status, error));
  std::optional<OriginIdentity> origin;
  if (origin_name != nullptr) {
    origin = OriginIdentity{std::string(origin_name),
                            optional_string(origin_version),
                            optional_string(origin_uri)};
  }
  return Revision{uuid(id),     sequence,          uuid(transaction_id),
                  committed_at, std::move(origin), optional_string(message)};
}

inline Result<std::vector<Revision>>
revisions(const pp_revision_set_t *revisions) {
  std::vector<Revision> result;
  const std::uint64_t count = pp_revision_set_count(revisions);
  result.reserve(static_cast<std::size_t>(count));
  for (std::uint64_t index = 0; index < count; ++index) {
    POSTPROJECT_TRY_ASSIGN(auto item_4, revision(revisions, index));
    result.push_back(std::move(item_4));
  }
  return result;
}

inline Result<std::string> required_event_string(const char *value,
                                                 std::string_view label) {
  if (value == nullptr) {
    return Error(ErrorCode::internal,
                 "revision event is missing " + std::string(label));
  }
  return std::string(value);
}

inline Result<RevisionEvent>
revision_event(const pp_revision_event_set_t *events, std::uint64_t index) {
  pp_revision_event_t event{};
  pp_error_t *error = nullptr;
  const pp_error_code_t status =
      pp_revision_event_set_get(events, index, &event, &error);
  POSTPROJECT_TRY(check(status, error));
  switch (event.kind) {
  case PP_REVISION_ASSET_IMPORTED:
    return RevisionEvent{event.position,
                         AssetImportedEvent{uuid(event.asset_id)}};
  case PP_REVISION_REPRESENTATION_ADDED:
    return RevisionEvent{event.position, RepresentationAddedEvent{
                                             uuid(event.asset_id),
                                             uuid(event.representation_id)}};
  case PP_REVISION_RESOURCE_ADDED:
    return RevisionEvent{event.position,
                         ResourceAddedEvent{uuid(event.resource_id)}};
  case PP_REVISION_REPRESENTATION_RESOURCE_ADDED:
    return RevisionEvent{event.position, RepresentationResourceAddedEvent{
                                             uuid(event.representation_id),
                                             uuid(event.resource_id),
                                             event.structural_position}};
  case PP_REVISION_LOCATOR_ADDED:
    return RevisionEvent{
        event.position,
        LocatorAddedEvent{uuid(event.resource_id), uuid(event.locator_id)}};
  case PP_REVISION_MEDIA_ROOT_ADDED:
    return RevisionEvent{event.position,
                         MediaRootAddedEvent{uuid(event.media_root_id)}};
  case PP_REVISION_LOCATOR_RETIRED:
    return RevisionEvent{
        event.position,
        LocatorRetiredEvent{uuid(event.resource_id), uuid(event.locator_id)}};
  case PP_REVISION_MEDIA_ROOT_ENABLED_CHANGED:
    return RevisionEvent{event.position,
                         MediaRootEnabledChangedEvent{uuid(event.media_root_id),
                                                      event.enabled != 0}};
  case PP_REVISION_MEDIA_ROOT_REMOVED:
    return RevisionEvent{event.position,
                         MediaRootRemovedEvent{uuid(event.media_root_id)}};
  case PP_REVISION_EXTERNAL_IDENTIFIER_ADDED: {
    POSTPROJECT_TRY_ASSIGN(
        std::string scheme,
        required_event_string(event.identifier_scheme, "identifier scheme"));
    POSTPROJECT_TRY_ASSIGN(
        std::string value,
        required_event_string(event.identifier_value, "identifier value"));
    return RevisionEvent{event.position,
                         ExternalIdentifierAddedEvent{
                             object_ref(event.target),
                             {std::move(scheme), std::move(value),
                              optional_string(event.identifier_qualifier)}}};
  }
  case PP_REVISION_EXTERNAL_IDENTIFIER_REMOVED: {
    POSTPROJECT_TRY_ASSIGN(
        std::string scheme,
        required_event_string(event.identifier_scheme, "identifier scheme"));
    POSTPROJECT_TRY_ASSIGN(
        std::string value,
        required_event_string(event.identifier_value, "identifier value"));
    return RevisionEvent{event.position,
                         ExternalIdentifierRemovedEvent{
                             object_ref(event.target),
                             {std::move(scheme), std::move(value),
                              optional_string(event.identifier_qualifier)}}};
  }
  case PP_REVISION_METADATA_ADDED_OR_REPLACED: {
    POSTPROJECT_TRY_ASSIGN(
        std::string vocabulary,
        required_event_string(event.vocabulary, "metadata vocabulary"));
    POSTPROJECT_TRY_ASSIGN(
        std::string property,
        required_event_string(event.property, "metadata property"));
    return RevisionEvent{event.position,
                         MetadataAddedOrReplacedEvent{object_ref(event.target),
                                                      std::move(vocabulary),
                                                      std::move(property)}};
  }
  case PP_REVISION_METADATA_REMOVED: {
    POSTPROJECT_TRY_ASSIGN(
        std::string vocabulary,
        required_event_string(event.vocabulary, "metadata vocabulary"));
    POSTPROJECT_TRY_ASSIGN(
        std::string property,
        required_event_string(event.property, "metadata property"));
    return RevisionEvent{event.position,
                         MetadataRemovedEvent{object_ref(event.target),
                                              std::move(vocabulary),
                                              std::move(property)}};
  }
  case PP_REVISION_ACTIVITY_CREATED: {
    POSTPROJECT_TRY_ASSIGN(
        std::string kind,
        required_event_string(event.activity_kind, "activity kind"));
    return RevisionEvent{
        event.position,
        ActivityCreatedEvent{uuid(event.activity_id), std::move(kind)}};
  }
  case PP_REVISION_ACTIVITY_INPUT_ADDED:
    return RevisionEvent{event.position,
                         ActivityInputAddedEvent{uuid(event.activity_id),
                                                 uuid(event.representation_id),
                                                 optional_string(event.role)}};
  case PP_REVISION_ACTIVITY_OUTPUT_ADDED:
    return RevisionEvent{event.position,
                         ActivityOutputAddedEvent{uuid(event.activity_id),
                                                  uuid(event.representation_id),
                                                  optional_string(event.role)}};
  case PP_REVISION_RESOURCE_FINGERPRINT_OBSERVED: {
    POSTPROJECT_TRY_ASSIGN(std::string algorithm,
                           required_event_string(event.fingerprint_algorithm,
                                                 "fingerprint algorithm"));
    return RevisionEvent{event.position,
                         ResourceFingerprintObservedEvent{
                             uuid(event.resource_id), std::move(algorithm),
                             event.fingerprint_version}};
  }
  case PP_REVISION_REPRESENTATION_FINGERPRINT_OBSERVED: {
    POSTPROJECT_TRY_ASSIGN(std::string algorithm,
                           required_event_string(event.fingerprint_algorithm,
                                                 "fingerprint algorithm"));
    return RevisionEvent{event.position,
                         RepresentationFingerprintObservedEvent{
                             uuid(event.representation_id),
                             std::move(algorithm), event.fingerprint_version}};
  }
  case PP_REVISION_DEPENDENCY_SET_RECORDED:
    return RevisionEvent{event.position, DependencySetRecordedEvent{
                                             uuid(event.representation_id)}};
  case PP_REVISION_JOB_REQUESTED:
    return RevisionEvent{event.position, JobRequestedEvent{uuid(event.job_id)}};
  case PP_REVISION_JOB_CLAIMED:
    return RevisionEvent{event.position, JobClaimedEvent{uuid(event.job_id)}};
  case PP_REVISION_JOB_CLAIM_RENEWED:
    return RevisionEvent{event.position,
                         JobClaimRenewedEvent{uuid(event.job_id)}};
  case PP_REVISION_JOB_CLAIM_RELEASED:
    return RevisionEvent{event.position,
                         JobClaimReleasedEvent{uuid(event.job_id)}};
  case PP_REVISION_JOB_SUCCEEDED:
    return RevisionEvent{event.position, JobSucceededEvent{uuid(event.job_id)}};
  case PP_REVISION_JOB_FAILED:
    return RevisionEvent{event.position, JobFailedEvent{uuid(event.job_id)}};
  case PP_REVISION_JOB_CANCELLED:
    return RevisionEvent{event.position, JobCancelledEvent{uuid(event.job_id)}};
  default:
    return Error(ErrorCode::internal,
                 "revision event has an unknown semantic kind");
  }
}

inline Result<Evidence>
resource_evidence(const pp_resolution_set_t *resolutions,
                  std::uint64_t representation_index,
                  std::uint64_t resource_index, std::uint64_t evidence_index) {
  pp_evidence_kind_t kind = 0;
  const char *detail = nullptr;
  pp_error_t *error = nullptr;
  const pp_error_code_t status = pp_resolution_set_get_resource_evidence(
      resolutions, representation_index, resource_index, evidence_index, &kind,
      &detail, &error);
  POSTPROJECT_TRY(check(status, error));
  return Evidence{static_cast<EvidenceKind>(kind),
                  detail != nullptr
                      ? std::optional<std::string>(std::string(detail))
                      : std::nullopt};
}

inline Result<Evidence>
candidate_evidence(const pp_resolution_set_t *resolutions,
                   std::uint64_t representation_index,
                   std::uint64_t resource_index, std::uint64_t candidate_index,
                   std::uint64_t evidence_index) {
  pp_evidence_kind_t kind = 0;
  const char *detail = nullptr;
  pp_error_t *error = nullptr;
  const pp_error_code_t status = pp_resolution_set_get_candidate_evidence(
      resolutions, representation_index, resource_index, candidate_index,
      evidence_index, &kind, &detail, &error);
  POSTPROJECT_TRY(check(status, error));
  return Evidence{static_cast<EvidenceKind>(kind),
                  detail != nullptr
                      ? std::optional<std::string>(std::string(detail))
                      : std::nullopt};
}

inline Result<ResourceResolution>
resolution_resource(const pp_resolution_set_t *resolutions,
                    std::uint64_t representation_index,
                    std::uint64_t resource_index) {
  pp_uuid_t resource_id{};
  pp_resource_resolution_state_t state = 0;
  std::uint64_t candidate_count = 0;
  std::uint64_t evidence_count = 0;
  pp_error_t *resource_error = nullptr;
  const pp_error_code_t resource_status = pp_resolution_set_get_resource(
      resolutions, representation_index, resource_index,
      &resource_id, &state, &candidate_count, &evidence_count,
      &resource_error);
  POSTPROJECT_TRY(check(resource_status, resource_error));

  std::vector<ResolutionCandidate> candidates;
  for (std::uint64_t candidate_index = 0;
       candidate_index < candidate_count; ++candidate_index) {
    const char *uri = nullptr;
    std::uint16_t confidence = 0;
    const char *media_root = nullptr;
    std::uint8_t has_naming = 0;
    pp_sequence_naming_t naming{};
    std::uint64_t candidate_evidence_count = 0;
    pp_error_t *candidate_error = nullptr;
    const pp_error_code_t candidate_status =
        pp_resolution_set_get_candidate(
            resolutions, representation_index, resource_index,
            candidate_index, &uri, &confidence, &media_root,
            &has_naming, &naming, &candidate_evidence_count,
            &candidate_error);
    POSTPROJECT_TRY(check(candidate_status, candidate_error));

    std::vector<Evidence> evidence;
    for (std::uint64_t evidence_index = 0;
         evidence_index < candidate_evidence_count; ++evidence_index) {
      POSTPROJECT_TRY_ASSIGN(
          auto item_5,
          candidate_evidence(resolutions,
                                     representation_index, resource_index,
                                     candidate_index, evidence_index));
      evidence.push_back(std::move(item_5));
    }
    candidates.push_back(
        {uri != nullptr ? std::string(uri) : std::string(), confidence,
         media_root != nullptr
             ? std::optional<std::string>(media_root)
             : std::nullopt,
         optional_naming(has_naming, naming),
         std::move(evidence)});
  }

  std::vector<Evidence> evidence;
  for (std::uint64_t evidence_index = 0;
       evidence_index < evidence_count; ++evidence_index) {
    POSTPROJECT_TRY_ASSIGN(
        auto item_6,
        resource_evidence(resolutions, representation_index,
                                  resource_index, evidence_index));
    evidence.push_back(std::move(item_6));
  }
  return ResourceResolution{uuid(resource_id),
                       static_cast<ResourceResolutionState>(state),
                       std::move(candidates), std::move(evidence)};
}

inline Result<std::vector<RepresentationResolution>>
resolution_values(ResolutionSetHandle resolutions) {
  std::vector<RepresentationResolution> result;
  const std::uint64_t count =
      pp_resolution_set_representation_count(resolutions.get());
  for (std::uint64_t representation_index = 0;
       representation_index < count; ++representation_index) {
    pp_uuid_t asset_id{};
    pp_uuid_t representation_id{};
    pp_representation_availability_t availability = 0;
    std::uint64_t resource_count = 0;
    std::uint64_t issue_count = 0;
    pp_error_t *item_error = nullptr;
    const pp_error_code_t item_status =
        pp_resolution_set_get_representation(
            resolutions.get(), representation_index, &asset_id,
            &representation_id, &availability, &resource_count,
            &issue_count, &item_error);
    POSTPROJECT_TRY(check(item_status, item_error));

    std::vector<ResourceResolution> resources;
    for (std::uint64_t resource_index = 0; resource_index < resource_count;
         ++resource_index) {
      POSTPROJECT_TRY_ASSIGN(
          ResourceResolution resource,
          resolution_resource(resolutions.get(), representation_index,
                                      resource_index));
      resources.push_back(std::move(resource));
    }

    std::vector<AvailabilityIssue> issues;
    for (std::uint64_t issue_index = 0; issue_index < issue_count;
         ++issue_index) {
      pp_uuid_t resource_id{};
      std::uint8_t required = 0;
      pp_availability_issue_kind_t kind = 0;
      std::uint64_t frame_count = 0;
      pp_error_t *issue_error = nullptr;
      const pp_error_code_t issue_status = pp_resolution_set_get_issue(
          resolutions.get(), representation_index, issue_index, &resource_id,
          &required, &kind, &frame_count, &issue_error);
      POSTPROJECT_TRY(check(issue_status, issue_error));
      std::vector<std::int64_t> frames;
      for (std::uint64_t frame_index = 0; frame_index < frame_count;
           ++frame_index) {
        std::int64_t frame = 0;
        pp_error_t *frame_error = nullptr;
        const pp_error_code_t frame_status =
            pp_resolution_set_get_issue_frame(
                resolutions.get(), representation_index, issue_index,
                frame_index, &frame, &frame_error);
        POSTPROJECT_TRY(check(frame_status, frame_error));
        frames.push_back(frame);
      }
      issues.push_back({uuid(resource_id), required != 0,
                        static_cast<AvailabilityIssueKind>(kind),
                        std::move(frames)});
    }
    result.push_back({uuid(asset_id), uuid(representation_id),
                      static_cast<RepresentationAvailability>(availability),
                      std::move(resources), std::move(issues)});
  }
  return result;
}

inline std::vector<ArtifactDependencyPathSegment>
artifact_dependency_path(const pp_artifact_reason_t &native) {
  std::vector<ArtifactDependencyPathSegment> dependency_path;
  dependency_path.reserve(
      static_cast<std::size_t>(native.dependency_path_length));
  for (std::uint64_t path_index = 0;
       path_index < native.dependency_path_length; ++path_index) {
    const pp_artifact_dependency_path_segment_t &segment =
        native.dependency_path[path_index];
    dependency_path.push_back(
        {uuid(segment.source_representation_id),
         segment.dependency_position,
         segment.has_source_resource != 0
             ? std::optional<Uuid>(uuid(segment.source_resource_id))
             : std::nullopt,
         std::string(segment.kind), object_ref(segment.target),
         segment.has_resolved_representation != 0
             ? std::optional<Uuid>(
                   uuid(segment.resolved_representation_id))
             : std::nullopt,
         std::string(segment.authored_reference)});
  }
  return dependency_path;
}

inline Result<ArtifactReason> artifact_reason(const pp_artifact_reason_t &native) {
  const auto kind = static_cast<ArtifactReasonKind>(native.kind);
  const bool has_dependency =
      kind == ArtifactReasonKind::dependency_snapshot_absent ||
      kind == ArtifactReasonKind::dependency_knowledge_incomplete ||
      kind == ArtifactReasonKind::dependency_path_changed ||
      kind == ArtifactReasonKind::dependency_fingerprint_changed ||
      kind == ArtifactReasonKind::
                  dependency_fingerprint_recomputation_pending ||
      kind == ArtifactReasonKind::
                  dependency_fingerprint_evidence_missing;
  const bool has_activity =
      kind == ArtifactReasonKind::snapshot_absent ||
      kind == ArtifactReasonKind::fingerprint_evidence_missing ||
      kind == ArtifactReasonKind::fingerprint_changed ||
      kind == ArtifactReasonKind::fingerprint_recomputation_pending ||
      has_dependency;
  const bool has_edge = has_activity && !has_dependency;
  auto dependency_path = artifact_dependency_path(native);
  ArtifactReason reason{
      kind,
       has_activity ? std::optional<Uuid>(uuid(native.activity_id))
                    : std::nullopt,
       uuid(native.representation_id),
       has_dependency ? std::optional<Uuid>(
                            uuid(native.input_representation_id))
                      : std::nullopt,
       has_edge ? std::optional<ArtifactEdgeKind>(
                      static_cast<ArtifactEdgeKind>(native.edge_kind))
                : std::nullopt,
       kind == ArtifactReasonKind::upstream_not_current
           ? std::optional<ArtifactKnowledgeState>(
                 static_cast<ArtifactKnowledgeState>(native.upstream_state))
           : std::nullopt,
       kind == ArtifactReasonKind::traversal_truncated
           ? std::optional<ArtifactTraversalLimit>(
                 static_cast<ArtifactTraversalLimit>(
                     native.traversal_limit))
           : std::nullopt,
       kind == ArtifactReasonKind::producing_activity_ambiguous
           ? std::optional<std::uint32_t>(native.activity_count)
           : std::nullopt,
       kind == ArtifactReasonKind::dependency_knowledge_incomplete
           ? std::optional<ArtifactDependencyIssue>(
                 static_cast<ArtifactDependencyIssue>(
                     native.dependency_issue))
           : std::nullopt,
       std::move(dependency_path),
       optional_string(native.fingerprint_algorithm),
       native.fingerprint_algorithm != nullptr
           ? std::optional<std::uint16_t>(native.fingerprint_version)
           : std::nullopt,
       std::nullopt, std::nullopt};
  POSTPROJECT_TRY_ASSIGN(
      reason.snapshot_value,
      optional_bytes(native.has_snapshot_value,
                             native.snapshot_value,
                             native.snapshot_value_length));
  POSTPROJECT_TRY_ASSIGN(
      reason.current_value,
      optional_bytes(native.has_current_value, native.current_value,
                             native.current_value_length));
  return reason;
}

inline Result<ArtifactEvaluation> artifact_evaluation(ArtifactEvaluationHandle evaluation) {
  pp_uuid_t evaluated_id{};
  pp_artifact_knowledge_state_t state = 0;
  std::uint32_t visited_representations = 0;
  std::uint8_t truncated = 0;
  std::uint64_t reason_count = 0;
  pp_error_t *summary_error = nullptr;
  const pp_error_code_t summary_status = pp_artifact_evaluation_get(
      evaluation.get(), &evaluated_id, &state, &visited_representations,
      &truncated, &reason_count, &summary_error);
  POSTPROJECT_TRY(check(summary_status, summary_error));

  std::vector<ArtifactReason> reasons;
  reasons.reserve(static_cast<std::size_t>(reason_count));
  for (std::uint64_t index = 0; index < reason_count; ++index) {
    pp_artifact_reason_t native{};
    pp_error_t *reason_error = nullptr;
    const pp_error_code_t reason_status = pp_artifact_evaluation_get_reason(
        evaluation.get(), index, &native, &reason_error);
    POSTPROJECT_TRY(check(reason_status, reason_error));
    POSTPROJECT_TRY_ASSIGN(ArtifactReason reason, artifact_reason(native));
    reasons.push_back(std::move(reason));
  }
  return ArtifactEvaluation{
      uuid(evaluated_id), static_cast<ArtifactKnowledgeState>(state),
      visited_representations, truncated != 0, std::move(reasons)};
}

inline Result<ArtifactReproducibility> artifact_reproducibility(ArtifactReproducibilityHandle report) {
  pp_uuid_t reported_id{};
  std::uint8_t reproducible = 0;
  std::uint8_t has_activity = 0;
  pp_uuid_t activity_id{};
  const char *activity_kind = nullptr;
  std::uint64_t issue_count = 0;
  pp_error_t *summary_error = nullptr;
  const pp_error_code_t summary_status = pp_artifact_reproducibility_get(
      report.get(), &reported_id, &reproducible, &has_activity, &activity_id,
      &activity_kind, &issue_count, &summary_error);
  POSTPROJECT_TRY(check(summary_status, summary_error));

  std::vector<ArtifactReproducibilityIssue> issues;
  issues.reserve(static_cast<std::size_t>(issue_count));
  for (std::uint64_t index = 0; index < issue_count; ++index) {
    pp_artifact_reproducibility_issue_t native{};
    pp_error_t *issue_error = nullptr;
    const pp_error_code_t issue_status =
        pp_artifact_reproducibility_get_issue(report.get(), index, &native,
                                              &issue_error);
    POSTPROJECT_TRY(check(issue_status, issue_error));
    const auto kind =
        static_cast<ArtifactReproducibilityIssueKind>(native.kind);
    const bool issue_has_activity =
        kind == ArtifactReproducibilityIssueKind::tool_identity_missing ||
        kind == ArtifactReproducibilityIssueKind::parameters_missing ||
        kind ==
            ArtifactReproducibilityIssueKind::input_representation_missing;
    issues.push_back(
        {kind,
         issue_has_activity
             ? std::optional<Uuid>(uuid(native.activity_id))
             : std::nullopt,
         kind ==
                 ArtifactReproducibilityIssueKind::input_representation_missing
             ? std::optional<Uuid>(uuid(native.representation_id))
             : std::nullopt,
         kind == ArtifactReproducibilityIssueKind::producing_activity_ambiguous
             ? std::optional<std::uint32_t>(native.activity_count)
             : std::nullopt});
  }
  return ArtifactReproducibility{
      uuid(reported_id), reproducible != 0,
      has_activity != 0 ? std::optional<Uuid>(uuid(activity_id))
                        : std::nullopt,
      optional_string(activity_kind), std::move(issues)};
}

inline Result<QueryPage<Job>> job_page(JobSetHandle jobs) {
  std::vector<Job> items;
  const std::uint64_t count = pp_job_set_count(jobs.get());
  items.reserve(static_cast<std::size_t>(count));
  for (std::uint64_t index = 0; index < count; ++index) {
    POSTPROJECT_TRY_ASSIGN(auto item_11, job(jobs.get(), index));
    items.push_back(std::move(item_11));
  }
  const char *next_cursor = pp_job_set_next_cursor(jobs.get());
  return QueryPage<Job>{std::move(items),
                        optional_string(next_cursor), false};
}

inline pp_decision_base_t native_decision_base(const DecisionBase &base) {
  pp_decision_base_t native{};
  native.production_id = native_production_id(base.production_id);
  if (base.revision) {
    native.has_revision = 1;
    native.revision_id = native_uuid(base.revision->id);
    native.revision_sequence = base.revision->sequence;
  }
  return native;
}

inline DecisionBase decision_base(const pp_decision_base_t &base) {
  DecisionBase result{production_id(base.production_id), std::nullopt};
  if (base.has_revision)
    result.revision = CommittedRevision{uuid(base.revision_id), base.revision_sequence};
  return result;
}

} // namespace detail

inline Result<DecisionBase> DecisionBase::fromToken(std::string_view token) {
  POSTPROJECT_TRY_ASSIGN(const std::string text,
                         detail::checked_string(token, "decision-base token"));
  pp_decision_base_t base{};
  pp_error_t *error = nullptr;
  const auto status = pp_decision_base_parse(text.c_str(), &base, &error);
  POSTPROJECT_TRY(detail::check(status, error));
  return detail::decision_base(base);
}

inline Result<std::string> DecisionBase::toToken() const {
  const auto native = detail::native_decision_base(*this);
  char *text = nullptr;
  pp_error_t *error = nullptr;
  const auto status = pp_decision_base_format(&native, &text, &error);
  detail::StringHandle owned(text);
  POSTPROJECT_TRY(detail::check(status, error));
  return std::string(text);
}

inline Result<ProductionId> ProductionId::fromString(std::string_view text) {
  POSTPROJECT_TRY_ASSIGN(const std::string checked,
                         detail::checked_string(text, "production ID"));
  pp_production_id_t id{};
  pp_error_t *error = nullptr;
  const auto status = pp_production_id_parse(checked.c_str(), &id, &error);
  POSTPROJECT_TRY(detail::check(status, error));
  Uuid::Bytes bytes{};
  std::copy(std::begin(id.bytes), std::end(id.bytes), bytes.begin());
  return ProductionId(bytes);
}

inline Result<std::string> ProductionId::toString() const {
  pp_production_id_t id{};
  std::copy(bytes().begin(), bytes().end(), std::begin(id.bytes));
  char *text = nullptr;
  pp_error_t *error = nullptr;
  const auto status = pp_production_id_format(id, &text, &error);
  detail::StringHandle owned(text);
  POSTPROJECT_TRY(detail::check(status, error));
  return std::string(owned.get());
}

inline Result<std::string> HostObjectBinding::toString() const {
  const auto native_production_id = detail::native_production_id(production_id);
  const pp_object_ref_t native_object = detail::native_object_ref(object);
  char *binding = nullptr;
  pp_error_t *error = nullptr;
  const pp_error_code_t status = pp_host_binding_format(
      native_production_id, &native_object, &binding, &error);
  POSTPROJECT_TRY(detail::check(status, error));
  detail::StringHandle owned(binding);
  return owned != nullptr ? std::string(owned.get()) : std::string();
}

inline Result<HostObjectBinding>
HostObjectBinding::fromString(std::string_view value) {
  POSTPROJECT_TRY_ASSIGN(const std::string checked,
                         detail::checked_string(value, "host binding"));
  pp_production_id_t production_id{};
  pp_object_ref_t object{};
  pp_error_t *error = nullptr;
  const pp_error_code_t status = pp_host_binding_parse(
      checked.c_str(), &production_id, &object, &error);
  POSTPROJECT_TRY(detail::check(status, error));
  return HostObjectBinding{detail::production_id(production_id),
                           detail::object_ref(object)};
}

class MetadataValue;
struct MetadataField;

// The alternatives a metadata value can hold. Each mirrors one value kind of
// the C ABI and compares by value.
struct MetadataString final {
  std::string value;
};

struct MetadataLanguageString final {
  std::string value;
  // A BCP 47 language tag.
  std::string language;
};

struct MetadataSignedInteger final {
  std::int64_t value;
};

struct MetadataUnsignedInteger final {
  std::uint64_t value;
};

// The exact value coefficient * 10^-scale; the coefficient is base-ten text.
struct MetadataDecimal final {
  std::string coefficient;
  std::uint32_t scale;
};

struct MetadataBoolean final {
  bool value;
};

struct MetadataTimestamp final {
  std::int64_t unix_micros;
};

struct MetadataUri final {
  std::string value;
};

struct MetadataBytes final {
  std::vector<std::uint8_t> value;
};

struct MetadataRational final {
  std::int64_t numerator;
  std::uint64_t denominator;
};

struct MetadataList final {
  std::vector<MetadataValue> items;
};

struct MetadataStructure final {
  std::vector<MetadataField> fields;
};

struct MetadataReference final {
  ObjectRef target;
};

// A typed metadata value that is both written and read. Construct one with a
// factory, read it with getIf or by visiting variant(). Values are validated
// when a call consumes them, which then returns any validation error.
class MetadataValue final {
public:
  using Variant =
      std::variant<MetadataString, MetadataLanguageString,
                   MetadataSignedInteger, MetadataUnsignedInteger,
                   MetadataDecimal, MetadataBoolean, MetadataTimestamp,
                   MetadataUri, MetadataBytes, MetadataRational, MetadataList,
                   MetadataStructure, MetadataReference>;

  explicit MetadataValue(Variant value);

  [[nodiscard]] static MetadataValue plainString(std::string_view value);

  [[nodiscard]] static MetadataValue languageString(std::string_view value,
                                                    std::string_view language);

  [[nodiscard]] static MetadataValue signedInteger(std::int64_t value);

  [[nodiscard]] static MetadataValue unsignedInteger(std::uint64_t value);

  [[nodiscard]] static MetadataValue decimal(std::string_view coefficient,
                                             std::uint32_t scale);

  [[nodiscard]] static MetadataValue boolean(bool value);

  [[nodiscard]] static MetadataValue timestamp(std::int64_t unix_micros);

  [[nodiscard]] static MetadataValue uri(std::string_view value);

  [[nodiscard]] static MetadataValue bytes(std::vector<std::uint8_t> value);

  [[nodiscard]] static MetadataValue rational(std::int64_t numerator,
                                              std::uint64_t denominator);

  [[nodiscard]] static MetadataValue list(std::vector<MetadataValue> items);

  [[nodiscard]] static MetadataValue
  structure(std::vector<MetadataField> fields);

  [[nodiscard]] static MetadataValue reference(const ObjectRef &target);

  // The held alternative, for std::visit or std::holds_alternative.
  [[nodiscard]] const Variant &variant() const noexcept { return value_; }

  // The held alternative if it is a T, otherwise nullptr.
  template <typename T> [[nodiscard]] const T *getIf() const noexcept {
    return std::get_if<T>(&value_);
  }

private:
  Variant value_;
};

struct MetadataField final {
  std::string name;
  MetadataValue value;
};

// Defined once MetadataField is complete, because constructing a value
// instantiates the variant's members for every alternative.
inline MetadataValue::MetadataValue(Variant value) : value_(std::move(value)) {}

inline MetadataValue MetadataValue::plainString(std::string_view value) {
  return MetadataValue(MetadataString{std::string(value)});
}

inline MetadataValue MetadataValue::languageString(std::string_view value,
                                                   std::string_view language) {
  return MetadataValue(
      MetadataLanguageString{std::string(value), std::string(language)});
}

inline MetadataValue MetadataValue::signedInteger(std::int64_t value) {
  return MetadataValue(MetadataSignedInteger{value});
}

inline MetadataValue MetadataValue::unsignedInteger(std::uint64_t value) {
  return MetadataValue(MetadataUnsignedInteger{value});
}

inline MetadataValue MetadataValue::decimal(std::string_view coefficient,
                                            std::uint32_t scale) {
  return MetadataValue(MetadataDecimal{std::string(coefficient), scale});
}

inline MetadataValue MetadataValue::boolean(bool value) {
  return MetadataValue(MetadataBoolean{value});
}

inline MetadataValue MetadataValue::timestamp(std::int64_t unix_micros) {
  return MetadataValue(MetadataTimestamp{unix_micros});
}

inline MetadataValue MetadataValue::uri(std::string_view value) {
  return MetadataValue(MetadataUri{std::string(value)});
}

inline MetadataValue MetadataValue::bytes(std::vector<std::uint8_t> value) {
  return MetadataValue(MetadataBytes{std::move(value)});
}

inline MetadataValue MetadataValue::rational(std::int64_t numerator,
                                             std::uint64_t denominator) {
  return MetadataValue(MetadataRational{numerator, denominator});
}

inline MetadataValue MetadataValue::reference(const ObjectRef &target) {
  return MetadataValue(MetadataReference{target});
}

inline MetadataValue MetadataValue::list(std::vector<MetadataValue> items) {
  return MetadataValue(MetadataList{std::move(items)});
}

inline MetadataValue
MetadataValue::structure(std::vector<MetadataField> fields) {
  return MetadataValue(MetadataStructure{std::move(fields)});
}

inline bool operator==(const MetadataString &left,
                       const MetadataString &right) {
  return left.value == right.value;
}
inline bool operator==(const MetadataLanguageString &left,
                       const MetadataLanguageString &right) {
  return left.value == right.value && left.language == right.language;
}
inline bool operator==(const MetadataSignedInteger &left,
                       const MetadataSignedInteger &right) {
  return left.value == right.value;
}
inline bool operator==(const MetadataUnsignedInteger &left,
                       const MetadataUnsignedInteger &right) {
  return left.value == right.value;
}
inline bool operator==(const MetadataDecimal &left,
                       const MetadataDecimal &right) {
  return left.coefficient == right.coefficient && left.scale == right.scale;
}
inline bool operator==(const MetadataBoolean &left,
                       const MetadataBoolean &right) {
  return left.value == right.value;
}
inline bool operator==(const MetadataTimestamp &left,
                       const MetadataTimestamp &right) {
  return left.unix_micros == right.unix_micros;
}
inline bool operator==(const MetadataUri &left, const MetadataUri &right) {
  return left.value == right.value;
}
inline bool operator==(const MetadataBytes &left, const MetadataBytes &right) {
  return left.value == right.value;
}
inline bool operator==(const MetadataRational &left,
                       const MetadataRational &right) {
  return left.numerator == right.numerator &&
         left.denominator == right.denominator;
}
inline bool operator==(const MetadataReference &left,
                       const MetadataReference &right) {
  return left.target == right.target;
}
// Recursive alternatives compare through MetadataValue, so all are declared
// before std::variant's comparison needs them.
inline bool operator==(const MetadataList &left, const MetadataList &right);
inline bool operator==(const MetadataField &left, const MetadataField &right);
inline bool operator==(const MetadataStructure &left,
                       const MetadataStructure &right);
inline bool operator==(const MetadataValue &left, const MetadataValue &right) {
  return left.variant() == right.variant();
}
inline bool operator!=(const MetadataValue &left, const MetadataValue &right) {
  return !(left == right);
}
inline bool operator==(const MetadataList &left, const MetadataList &right) {
  return left.items == right.items;
}
inline bool operator==(const MetadataField &left, const MetadataField &right) {
  return left.name == right.name && left.value == right.value;
}
inline bool operator==(const MetadataStructure &left,
                       const MetadataStructure &right) {
  return left.fields == right.fields;
}

namespace detail {

struct MetadataInputDeleter final {
  void operator()(pp_metadata_input_t *input) const noexcept {
    pp_metadata_input_release(input);
  }
};

using MetadataInputHandle =
    std::unique_ptr<pp_metadata_input_t, MetadataInputDeleter>;

inline Result<MetadataInputHandle>
checked_metadata_input(pp_error_code_t status, pp_metadata_input_t *input,
                       pp_error_t *error) {
  MetadataInputHandle handle(input);
  POSTPROJECT_TRY(check(status, error));
  return handle;
}

// Builds the native input a call consumes, validating the value.
inline Result<MetadataInputHandle>
native_metadata_input(const MetadataValue &value) {
  pp_metadata_input_t *input = nullptr;
  pp_error_t *error = nullptr;
  if (const auto *text = value.getIf<MetadataString>()) {
    POSTPROJECT_TRY_ASSIGN(const std::string native,
                           checked_string(text->value, "metadata string"));
    const pp_error_code_t status = pp_metadata_input_create_string(
        native.c_str(), nullptr, &input, &error);
    return checked_metadata_input(status, input, error);
  }
  if (const auto *text = value.getIf<MetadataLanguageString>()) {
    POSTPROJECT_TRY_ASSIGN(const std::string native,
                           checked_string(text->value, "metadata string"));
    POSTPROJECT_TRY_ASSIGN(const std::string language,
                           checked_string(text->language, "metadata language"));
    const pp_error_code_t status = pp_metadata_input_create_string(
        native.c_str(), language.c_str(), &input, &error);
    return checked_metadata_input(status, input, error);
  }
  if (const auto *number = value.getIf<MetadataSignedInteger>()) {
    const pp_error_code_t status =
        pp_metadata_input_create_i64(number->value, &input, &error);
    return checked_metadata_input(status, input, error);
  }
  if (const auto *number = value.getIf<MetadataUnsignedInteger>()) {
    const pp_error_code_t status =
        pp_metadata_input_create_u64(number->value, &input, &error);
    return checked_metadata_input(status, input, error);
  }
  if (const auto *number = value.getIf<MetadataDecimal>()) {
    POSTPROJECT_TRY_ASSIGN(
        const std::string coefficient,
        checked_string(number->coefficient, "decimal coefficient"));
    const pp_error_code_t status = pp_metadata_input_create_decimal(
        coefficient.c_str(), number->scale, &input, &error);
    return checked_metadata_input(status, input, error);
  }
  if (const auto *flag = value.getIf<MetadataBoolean>()) {
    const pp_error_code_t status =
        pp_metadata_input_create_bool(flag->value ? 1 : 0, &input, &error);
    return checked_metadata_input(status, input, error);
  }
  if (const auto *time = value.getIf<MetadataTimestamp>()) {
    const pp_error_code_t status =
        pp_metadata_input_create_timestamp(time->unix_micros, &input, &error);
    return checked_metadata_input(status, input, error);
  }
  if (const auto *uri = value.getIf<MetadataUri>()) {
    POSTPROJECT_TRY_ASSIGN(const std::string native,
                           checked_string(uri->value, "metadata URI"));
    const pp_error_code_t status =
        pp_metadata_input_create_uri(native.c_str(), &input, &error);
    return checked_metadata_input(status, input, error);
  }
  if (const auto *bytes = value.getIf<MetadataBytes>()) {
    const pp_error_code_t status = pp_metadata_input_create_bytes(
        bytes->value.data(), static_cast<std::uint64_t>(bytes->value.size()),
        &input, &error);
    return checked_metadata_input(status, input, error);
  }
  if (const auto *ratio = value.getIf<MetadataRational>()) {
    const pp_error_code_t status = pp_metadata_input_create_rational(
        ratio->numerator, ratio->denominator, &input, &error);
    return checked_metadata_input(status, input, error);
  }
  if (const auto *list = value.getIf<MetadataList>()) {
    std::vector<MetadataInputHandle> items;
    items.reserve(list->items.size());
    std::vector<const pp_metadata_input_t *> native_items;
    native_items.reserve(list->items.size());
    for (const MetadataValue &item : list->items) {
      POSTPROJECT_TRY_ASSIGN(MetadataInputHandle native,
                             native_metadata_input(item));
      native_items.push_back(native.get());
      items.push_back(std::move(native));
    }
    const pp_error_code_t status = pp_metadata_input_create_list(
        native_items.data(), static_cast<std::uint64_t>(native_items.size()),
        &input, &error);
    return checked_metadata_input(status, input, error);
  }
  if (const auto *structure = value.getIf<MetadataStructure>()) {
    std::vector<std::string> names;
    names.reserve(structure->fields.size());
    std::vector<MetadataInputHandle> fields;
    fields.reserve(structure->fields.size());
    for (const MetadataField &field : structure->fields) {
      POSTPROJECT_TRY_ASSIGN(std::string name,
                             checked_string(field.name, "metadata field name"));
      names.push_back(std::move(name));
      POSTPROJECT_TRY_ASSIGN(MetadataInputHandle native,
                             native_metadata_input(field.value));
      fields.push_back(std::move(native));
    }
    std::vector<const char *> name_pointers;
    name_pointers.reserve(names.size());
    for (const std::string &name : names) {
      name_pointers.push_back(name.c_str());
    }
    std::vector<const pp_metadata_input_t *> native_fields;
    native_fields.reserve(fields.size());
    for (const MetadataInputHandle &field : fields) {
      native_fields.push_back(field.get());
    }
    const pp_error_code_t status = pp_metadata_input_create_struct(
        name_pointers.data(), native_fields.data(),
        static_cast<std::uint64_t>(native_fields.size()), &input, &error);
    return checked_metadata_input(status, input, error);
  }
  const auto &reference = std::get<MetadataReference>(value.variant());
  const pp_object_ref_t target = native_object_ref(reference.target);
  const pp_error_code_t status =
      pp_metadata_input_create_reference(&target, &input, &error);
  return checked_metadata_input(status, input, error);
}

} // namespace detail

struct RegenerationParameter final {
  std::string vocabulary;
  std::string property;
  MetadataValue value;
};

struct MetadataAssertion final {
  ObjectRef target;
  std::string vocabulary;
  std::string property;
  MetadataValue value;
};

struct RegenerationJobPlan final {
  Uuid artifact_representation_id;
  Job job;
  std::vector<RegenerationParameter> parameters;
};

namespace detail {

inline Result<MetadataValue> metadata_value(const pp_metadata_value_t *value) {
  pp_error_t *error = nullptr;
  switch (pp_metadata_value_kind(value)) {
  case PP_METADATA_STRING:
  case PP_METADATA_LANG_STRING: {
    const char *text = nullptr;
    const char *language = nullptr;
    const pp_error_code_t status =
        pp_metadata_value_get_string(value, &text, &language, &error);
    POSTPROJECT_TRY(check(status, error));
    if (language != nullptr) {
      return MetadataValue::languageString(text, language);
    }
    return MetadataValue::plainString(text);
  }
  case PP_METADATA_I64: {
    std::int64_t result = 0;
    const pp_error_code_t status =
        pp_metadata_value_get_i64(value, &result, &error);
    POSTPROJECT_TRY(check(status, error));
    return MetadataValue::signedInteger(result);
  }
  case PP_METADATA_U64: {
    std::uint64_t result = 0;
    const pp_error_code_t status =
        pp_metadata_value_get_u64(value, &result, &error);
    POSTPROJECT_TRY(check(status, error));
    return MetadataValue::unsignedInteger(result);
  }
  case PP_METADATA_DECIMAL: {
    const char *coefficient = nullptr;
    std::uint32_t scale = 0;
    const pp_error_code_t status = pp_metadata_value_get_decimal(
        value, &coefficient, &scale, &error);
    POSTPROJECT_TRY(check(status, error));
    return MetadataValue::decimal(coefficient, scale);
  }
  case PP_METADATA_BOOL: {
    std::uint8_t result = 0;
    const pp_error_code_t status =
        pp_metadata_value_get_bool(value, &result, &error);
    POSTPROJECT_TRY(check(status, error));
    return MetadataValue::boolean(result != 0);
  }
  case PP_METADATA_TIMESTAMP: {
    std::int64_t result = 0;
    const pp_error_code_t status =
        pp_metadata_value_get_timestamp(value, &result, &error);
    POSTPROJECT_TRY(check(status, error));
    return MetadataValue::timestamp(result);
  }
  case PP_METADATA_URI: {
    const char *result = nullptr;
    const pp_error_code_t status =
        pp_metadata_value_get_uri(value, &result, &error);
    POSTPROJECT_TRY(check(status, error));
    return MetadataValue::uri(result);
  }
  case PP_METADATA_BYTES: {
    const std::uint8_t *data = nullptr;
    std::uint64_t length = 0;
    const pp_error_code_t status =
        pp_metadata_value_get_bytes(value, &data, &length, &error);
    POSTPROJECT_TRY(check(status, error));
    std::vector<std::uint8_t> bytes;
    if (length != 0) {
      bytes.assign(data, data + length);
    }
    return MetadataValue::bytes(std::move(bytes));
  }
  case PP_METADATA_RATIONAL: {
    std::int64_t numerator = 0;
    std::uint64_t denominator = 0;
    const pp_error_code_t status = pp_metadata_value_get_rational(
        value, &numerator, &denominator, &error);
    POSTPROJECT_TRY(check(status, error));
    return MetadataValue::rational(numerator, denominator);
  }
  case PP_METADATA_LIST: {
    const std::uint64_t count = pp_metadata_value_list_count(value);
    std::vector<MetadataValue> items;
    items.reserve(static_cast<std::size_t>(count));
    for (std::uint64_t index = 0; index < count; ++index) {
      const pp_metadata_value_t *item = nullptr;
      error = nullptr;
      const pp_error_code_t status =
          pp_metadata_value_list_get(value, index, &item, &error);
      POSTPROJECT_TRY(check(status, error));
      POSTPROJECT_TRY_ASSIGN(MetadataValue item_value, metadata_value(item));
      items.push_back(std::move(item_value));
    }
    return MetadataValue::list(std::move(items));
  }
  case PP_METADATA_STRUCT: {
    const std::uint64_t count = pp_metadata_value_struct_count(value);
    std::vector<MetadataField> fields;
    fields.reserve(static_cast<std::size_t>(count));
    for (std::uint64_t index = 0; index < count; ++index) {
      const char *name = nullptr;
      const pp_metadata_value_t *field_value = nullptr;
      error = nullptr;
      const pp_error_code_t status = pp_metadata_value_struct_get(
          value, index, &name, &field_value, &error);
      POSTPROJECT_TRY(check(status, error));
      POSTPROJECT_TRY_ASSIGN(MetadataValue field, metadata_value(field_value));
      fields.push_back({std::string(name), std::move(field)});
    }
    return MetadataValue::structure(std::move(fields));
  }
  case PP_METADATA_REFERENCE: {
    pp_object_ref_t target{};
    const pp_error_code_t status =
        pp_metadata_value_get_reference(value, &target, &error);
    POSTPROJECT_TRY(check(status, error));
    return MetadataValue::reference(object_ref(target));
  }
  default:
    return Error(ErrorCode::internal, "unknown metadata value kind");
  }
}

inline Result<QueryPage<MetadataAssertion>>
metadata_page(MetadataSetHandle metadata) {
  std::vector<MetadataAssertion> items;
  const std::uint64_t count = pp_metadata_set_count(metadata.get());
  items.reserve(static_cast<std::size_t>(count));
  for (std::uint64_t index = 0; index < count; ++index) {
    pp_object_ref_t target{};
    const char *vocabulary = nullptr;
    const char *property = nullptr;
    const pp_metadata_value_t *value = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_metadata_set_get(
        metadata.get(), index, &target, &vocabulary, &property, &value, &error);
    POSTPROJECT_TRY(check(status, error));
    if (vocabulary == nullptr || property == nullptr || value == nullptr) {
      return Error(ErrorCode::internal, "metadata assertion is incomplete");
    }
    POSTPROJECT_TRY_ASSIGN(MetadataValue item, metadata_value(value));
    items.push_back({object_ref(target), std::string(vocabulary),
                     std::string(property), std::move(item)});
  }
  const char *cursor = pp_metadata_set_next_cursor(metadata.get());
  return QueryPage<MetadataAssertion>{std::move(items), optional_string(cursor),
                                      false};
}

} // namespace detail

// Move-only and caller-serialized. Do not call one Transaction concurrently.
// Blocks until revisions after a sequence exist. Owns its own connection, so a
// wait never holds the production. Waits are caller-serialized; cancel() may
// be called from any thread. Destroying the production closes the waiter.
class RevisionWaiter final {
public:
  RevisionWaiter(const RevisionWaiter &) = delete;
  RevisionWaiter &operator=(const RevisionWaiter &) = delete;

  RevisionWaiter(RevisionWaiter &&other) noexcept
      : waiter_(std::exchange(other.waiter_, nullptr)) {}

  RevisionWaiter &operator=(RevisionWaiter &&other) noexcept {
    if (this != &other) {
      pp_revision_waiter_release(waiter_);
      waiter_ = std::exchange(other.waiter_, nullptr);
    }
    return *this;
  }

  ~RevisionWaiter() { pp_revision_waiter_release(waiter_); }

  // Returns up to `limit` revisions after `after_sequence`, or a timed-out,
  // closed, or cancelled result. Closed and cancelled are terminal.
  [[nodiscard]] Result<RevisionWait>
  wait(std::uint64_t after_sequence, std::uint32_t limit = 100,
       std::chrono::milliseconds timeout = max_revision_wait) {
    if (timeout.count() < 0 || timeout > max_revision_wait) {
      return Error(ErrorCode::invalid_argument,
                   "revision wait timeout must be 0-60000 ms");
    }
    pp_revision_wait_result_t result = 0;
    pp_revision_set_t *raw_revisions = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_revision_waiter_wait(
        waiter_, after_sequence, limit,
        static_cast<std::uint32_t>(timeout.count()), &result, &raw_revisions,
        &error);
    POSTPROJECT_TRY(detail::check(status, error));
    detail::RevisionSetHandle revisions(raw_revisions);
    POSTPROJECT_TRY_ASSIGN(std::vector<Revision> items,
                           detail::revisions(revisions.get()));
    return RevisionWait{static_cast<RevisionWaitResult>(result),
                        std::move(items)};
  }

  // Ends the current wait; later waits return RevisionWaitResult::cancelled.
  void cancel() noexcept { pp_revision_waiter_cancel(waiter_); }

private:
  friend class Production;

  explicit RevisionWaiter(pp_revision_waiter_t *waiter) noexcept
      : waiter_(waiter) {}

  pp_revision_waiter_t *waiter_ = nullptr;
};

// The content structure of a representation at its present location: a
// single file, an image sequence, ordered parts, or a package. The same source
// serves Transaction::importMedia, which creates an asset whose original has
// the source's structure, and Transaction::addRepresentation. A UTF-8 path
// converts implicitly to a single-file source. Inputs are checked when a
// transaction uses the source.
class MediaSource final {
public:
  // Implicit: a path is a single-file source.
  MediaSource(std::string_view path) : content_(File{std::string(path)}) {}
  MediaSource(const std::string &path) : MediaSource(std::string_view(path)) {}
  MediaSource(const char *path)
      : MediaSource(path != nullptr ? std::string_view(path)
                                    : std::string_view()) {}

  [[nodiscard]] static MediaSource file(std::string_view path) {
    return MediaSource(path);
  }

  [[nodiscard]] static MediaSource imageSequence(ImageSequenceInput sequence) {
    return MediaSource(Content(std::move(sequence)));
  }

  // Parts keep their order and must all be required.
  [[nodiscard]] static MediaSource
  orderedParts(std::vector<FileResourceInput> parts) {
    return MediaSource(Content(Collection{std::move(parts), false}));
  }

  // A package needs at least one required member; optional members may be
  // missing without making the representation unavailable.
  [[nodiscard]] static MediaSource
  package(std::vector<FileResourceInput> members) {
    return MediaSource(Content(Collection{std::move(members), true}));
  }

private:
  friend class Transaction;

  struct File final {
    std::string path;
  };

  struct Collection final {
    std::vector<FileResourceInput> members;
    bool package;
  };

  using Content = std::variant<File, ImageSequenceInput, Collection>;

  explicit MediaSource(Content content) : content_(std::move(content)) {}

  Result<detail::MediaSourceHandle> native() const {
    pp_media_source_t *raw = nullptr;
    pp_error_t *error = nullptr;
    pp_error_code_t status = PP_OK;
    if (const File *file = std::get_if<File>(&content_)) {
      POSTPROJECT_TRY_ASSIGN(const std::string path,
                             detail::checked_string(file->path, "path"));
      status = pp_media_source_create_file(path.c_str(), &raw, &error);
    } else if (const ImageSequenceInput *sequence =
                   std::get_if<ImageSequenceInput>(&content_)) {
      POSTPROJECT_TRY_ASSIGN(
          const std::string directory,
          detail::checked_string(sequence->directory, "directory"));
      POSTPROJECT_TRY_ASSIGN(detail::NativeNaming naming,
                             detail::NativeNaming::make(sequence->naming));
      status = pp_media_source_create_image_sequence(
          directory.c_str(), naming.get(), sequence->start, sequence->end,
          sequence->step,
          sequence->rate_numerator, sequence->rate_denominator,
          sequence->missing_frames.data(),
          static_cast<std::uint64_t>(sequence->missing_frames.size()), &raw,
          &error);
    } else if (const Collection *collection =
                   std::get_if<Collection>(&content_)) {
      std::vector<std::string> paths;
      paths.reserve(collection->members.size());
      std::vector<std::string> roles;
      roles.reserve(collection->members.size());
      for (const FileResourceInput &member : collection->members) {
        POSTPROJECT_TRY_ASSIGN(
            std::string path,
            detail::checked_string(member.path, "member path"));
        paths.push_back(std::move(path));
        POSTPROJECT_TRY_ASSIGN(
            std::string role,
            detail::checked_string(member.role, "member role"));
        roles.push_back(std::move(role));
      }
      std::vector<pp_file_resource_input_t> members;
      members.reserve(collection->members.size());
      for (std::size_t index = 0; index < collection->members.size();
           ++index) {
        members.push_back(
            {paths[index].c_str(), roles[index].c_str(),
             static_cast<std::uint8_t>(
                 collection->members[index].required ? 1 : 0)});
      }
      const auto count = static_cast<std::uint64_t>(members.size());
      status = collection->package
                   ? pp_media_source_create_package(members.data(), count,
                                                    &raw, &error)
                   : pp_media_source_create_ordered_parts(
                         members.data(), count, &raw, &error);
    }
    detail::MediaSourceHandle handle(raw);
    POSTPROJECT_TRY(detail::check(status, error));
    return Result<detail::MediaSourceHandle>(std::move(handle));
  }

  Content content_;
};

class Transaction final {
public:
  Result<void> setRevisionContext(const RevisionContext &context) {
    std::optional<std::string> origin_name;
    if (context.origin.has_value()) {
      POSTPROJECT_TRY_ASSIGN(origin_name, checked_origin_name(*context.origin));
    }
    std::optional<std::string> origin_version;
    if (context.origin.has_value()) {
      POSTPROJECT_TRY_ASSIGN(origin_version,
                             detail::checked_optional_string(
                                 context.origin->version, "origin version"));
    }
    std::optional<std::string> origin_uri;
    if (context.origin.has_value()) {
      POSTPROJECT_TRY_ASSIGN(
          origin_uri,
          detail::checked_optional_string(context.origin->uri, "origin URI"));
    }
    POSTPROJECT_TRY_ASSIGN(
        const std::optional<std::string> message,
        detail::checked_optional_string(context.message, "revision message"));
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_transaction_set_revision_context(
        transaction_, origin_name.has_value() ? origin_name->c_str() : nullptr,
        origin_version.has_value() ? origin_version->c_str() : nullptr,
        origin_uri.has_value() ? origin_uri->c_str() : nullptr,
        message.has_value() ? message->c_str() : nullptr, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return {};
  }

  // Creates an asset whose original representation has the source's
  // structure. A path imports a single file.
  Result<Uuid> importMedia(const MediaSource &source) {
    return import_media_impl(source, nullptr);
  }

  Result<Uuid> importMedia(const MediaSource &source,
                           std::string_view display_name) {
    POSTPROJECT_TRY_ASSIGN(
        const std::string name,
        detail::checked_string(display_name, "display_name"));
    return import_media_impl(source, name.c_str());
  }

  // Adds a representation of the given kind, with the source's structure, to
  // an existing asset.
  Result<Uuid> addRepresentation(const Uuid &asset_id, RepresentationKind kind,
                                 const MediaSource &source) {
    POSTPROJECT_TRY_ASSIGN(const detail::MediaSourceHandle native_source,
                           source.native());
    const pp_uuid_t native_asset_id = detail::native_uuid(asset_id);
    pp_uuid_t value{};
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_transaction_add_representation(
        transaction_, &native_asset_id,
        static_cast<pp_representation_kind_t>(kind), native_source.get(),
        &value, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return detail::uuid(value);
  }

  Result<Uuid> addMediaRoot(std::string_view name, std::int32_t priority = 0) {
    return add_media_root_impl(name, nullptr, priority);
  }

  Result<Uuid> addMediaRoot(std::string_view name, std::string_view label,
                            std::int32_t priority = 0) {
    POSTPROJECT_TRY_ASSIGN(const std::string native_label,
                           detail::checked_string(label, "label"));
    return add_media_root_impl(name, native_label.c_str(), priority);
  }

  Result<void> setMediaRootEnabled(const Uuid &root_id, bool enabled) {
    const pp_uuid_t id = detail::native_uuid(root_id);
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_transaction_set_media_root_enabled(
        transaction_, &id, enabled ? UINT8_C(1) : UINT8_C(0), &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return {};
  }

  Result<void> removeMediaRoot(const Uuid &root_id) {
    const pp_uuid_t id = detail::native_uuid(root_id);
    pp_error_t *error = nullptr;
    const pp_error_code_t status =
        pp_transaction_remove_media_root(transaction_, &id, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return {};
  }

  // Confirms uri as a locator of the resource. root_name records the logical
  // media root it was found under. sequence_naming names the files at a
  // locator of an image-sequence resource: required for such a resource and
  // rejected at commit for any other.
  Result<void>
  confirmLocator(const Uuid &resource_id, std::string_view uri,
                 const std::optional<std::string> &root_name = std::nullopt,
                 const std::optional<SequenceNaming> &sequence_naming =
                     std::nullopt) {
    const pp_uuid_t id = detail::native_uuid(resource_id);
    POSTPROJECT_TRY_ASSIGN(const std::string native_uri,
                           detail::checked_string(uri, "uri"));
    POSTPROJECT_TRY_ASSIGN(
        const std::optional<std::string> native_root,
        detail::checked_optional_string(root_name, "root name"));
    POSTPROJECT_TRY_ASSIGN(detail::NativeNaming naming,
                           detail::NativeNaming::make(sequence_naming));
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_transaction_confirm_locator(
        transaction_, &id, native_uri.c_str(),
        detail::optional_c_str(native_root), naming.get(), &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return {};
  }

  Result<void> retireLocator(const Uuid &locator_id) {
    const pp_uuid_t id = detail::native_uuid(locator_id);
    pp_error_t *error = nullptr;
    const pp_error_code_t status =
        pp_transaction_retire_locator(transaction_, &id, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return {};
  }

  Result<void> recordResourceFingerprint(const Uuid &resource_id,
                                         const Fingerprint &fingerprint) {
    const pp_uuid_t id = detail::native_uuid(resource_id);
    POSTPROJECT_TRY_ASSIGN(
        const std::string algorithm,
        detail::checked_string(fingerprint.algorithm, "fingerprint algorithm"));
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_transaction_record_resource_fingerprint(
        transaction_, &id, algorithm.c_str(), fingerprint.version,
        fingerprint.value.data(),
        static_cast<std::uint64_t>(fingerprint.value.size()), &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return {};
  }

  Result<void> recordRepresentationFingerprint(const Uuid &representation_id,
                                               const Fingerprint &fingerprint) {
    const pp_uuid_t id = detail::native_uuid(representation_id);
    POSTPROJECT_TRY_ASSIGN(
        const std::string algorithm,
        detail::checked_string(fingerprint.algorithm, "fingerprint algorithm"));
    pp_error_t *error = nullptr;
    const pp_error_code_t status =
        pp_transaction_record_representation_fingerprint(
            transaction_, &id, algorithm.c_str(), fingerprint.version,
            fingerprint.value.data(),
            static_cast<std::uint64_t>(fingerprint.value.size()), &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return {};
  }

  // Fingerprints the content at path as the resource's present content and
  // stages it together with every representation recomputed from it. The
  // outcome says whether the content changed; unchanged content records no
  // fingerprint. For an image sequence, path is its directory and
  // sequence_naming names its files; empty means the naming recorded there.
  Result<ContentObservationOutcome> observeResourceContent(
      const Uuid &resource_id, std::string_view path,
      const std::optional<SequenceNaming> &sequence_naming = std::nullopt) {
    const pp_uuid_t id = detail::native_uuid(resource_id);
    POSTPROJECT_TRY_ASSIGN(const std::string native_path,
                           detail::checked_string(path, "path"));
    POSTPROJECT_TRY_ASSIGN(detail::NativeNaming naming,
                           detail::NativeNaming::make(sequence_naming));
    pp_content_observation_t outcome = 0;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_transaction_observe_resource_content(
        transaction_, &id, native_path.c_str(), naming.get(), &outcome,
        &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return static_cast<ContentObservationOutcome>(outcome);
  }

  Result<void>
  recordDependencySet(const Uuid &representation_id,
                      const std::vector<Dependency> &dependencies) {
    std::vector<std::string> kinds;
    std::vector<std::string> authored_references;
    kinds.reserve(dependencies.size());
    authored_references.reserve(dependencies.size());
    for (const Dependency &dependency : dependencies) {
      POSTPROJECT_TRY_ASSIGN(
          auto item_4,
          detail::checked_string(dependency.kind, "dependency kind"));
      kinds.push_back(std::move(item_4));
      POSTPROJECT_TRY_ASSIGN(
          auto item_5, detail::checked_string(dependency.authored_reference,
                                              "authored dependency reference"));
      authored_references.push_back(std::move(item_5));
    }

    std::vector<pp_dependency_t> native_dependencies;
    native_dependencies.reserve(dependencies.size());
    for (std::size_t index = 0; index < dependencies.size(); ++index) {
      const Dependency &dependency = dependencies[index];
      native_dependencies.push_back(
          {static_cast<std::uint8_t>(
               dependency.source_resource_id.has_value() ? 1 : 0),
           dependency.source_resource_id.has_value()
               ? detail::native_uuid(*dependency.source_resource_id)
               : pp_uuid_t{},
           kinds[index].c_str(), detail::native_object_ref(dependency.target),
           static_cast<std::uint8_t>(
               dependency.resolved_representation_id.has_value() ? 1 : 0),
           dependency.resolved_representation_id.has_value()
               ? detail::native_uuid(*dependency.resolved_representation_id)
               : pp_uuid_t{},
           static_cast<std::uint8_t>(dependency.required ? 1 : 0),
           authored_references[index].c_str()});
    }
    const pp_uuid_t native_id = detail::native_uuid(representation_id);
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_transaction_record_dependency_set(
        transaction_, &native_id,
        native_dependencies.empty() ? nullptr : native_dependencies.data(),
        static_cast<std::uint64_t>(native_dependencies.size()), &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return {};
  }

  Result<void> addExternalIdentifier(const ObjectRef &target,
                                     const ExternalIdentifier &identifier) {
    POSTPROJECT_TRY(mutate_external_identifier(false, target, identifier));
    return {};
  }

  Result<void> removeExternalIdentifier(const ObjectRef &target,
                                        const ExternalIdentifier &identifier) {
    POSTPROJECT_TRY(mutate_external_identifier(true, target, identifier));
    return {};
  }

  Result<void> addMetadataValue(const ObjectRef &target,
                                std::string_view vocabulary,
                                std::string_view property,
                                const MetadataValue &value) {
    const pp_object_ref_t native_target = detail::native_object_ref(target);
    POSTPROJECT_TRY_ASSIGN(
        const std::string native_vocabulary,
        detail::checked_string(vocabulary, "metadata vocabulary"));
    POSTPROJECT_TRY_ASSIGN(
        const std::string native_property,
        detail::checked_string(property, "metadata property"));
    POSTPROJECT_TRY_ASSIGN(const detail::MetadataInputHandle native_input,
                           detail::native_metadata_input(value));
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_transaction_add_metadata_value(
        transaction_, &native_target, native_vocabulary.c_str(),
        native_property.c_str(), native_input.get(), &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return {};
  }

  Result<Uuid> requestJob(const JobRequest &request) {
    POSTPROJECT_TRY_ASSIGN(const std::string kind,
                           detail::checked_string(request.kind, "job kind"));
    POSTPROJECT_TRY_ASSIGN(const std::optional<std::string> target_root,
                           detail::checked_optional_string(request.target_root,
                                                           "job target root"));
    std::vector<pp_uuid_t> inputs;
    inputs.reserve(request.inputs.size());
    for (const Uuid &input : request.inputs) {
      inputs.push_back(detail::native_uuid(input));
    }
    const pp_uuid_t output_asset_id =
        detail::native_uuid(request.output_asset_id);
    pp_uuid_t job_id{};
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_transaction_request_job(
        transaction_, kind.c_str(), inputs.empty() ? nullptr : inputs.data(),
        static_cast<std::uint64_t>(inputs.size()), &output_asset_id,
        static_cast<pp_representation_kind_t>(
            request.output_representation_kind),
        target_root.has_value() ? target_root->c_str() : nullptr, &job_id,
        &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return detail::uuid(job_id);
  }

  Result<Uuid> claimJob(const Uuid &job_id, const ToolIdentity &tool,
                        const std::optional<AgentIdentity> &agent,
                        std::int64_t now_unix_micros,
                        std::int64_t expires_at_unix_micros) {
    const pp_uuid_t native_job_id = detail::native_uuid(job_id);
    POSTPROJECT_TRY_ASSIGN(const std::string tool_name,
                           detail::checked_string(tool.name, "tool name"));
    POSTPROJECT_TRY_ASSIGN(
        const std::optional<std::string> tool_version,
        detail::checked_optional_string(tool.version, "tool version"));
    POSTPROJECT_TRY_ASSIGN(
        const std::optional<std::string> tool_uri,
        detail::checked_optional_string(tool.uri, "tool URI"));
    std::optional<std::string> agent_name;
    if (agent.has_value()) {
      POSTPROJECT_TRY_ASSIGN(agent_name, detail::checked_optional_string(
                                             agent->name, "agent name"));
    }
    const std::optional<ExternalIdentifier> identifier =
        agent.has_value() ? agent->identifier : std::nullopt;
    std::optional<std::string> agent_scheme;
    if (identifier.has_value()) {
      POSTPROJECT_TRY_ASSIGN(agent_scheme,
                             detail::checked_string(identifier->scheme,
                                                    "agent identifier scheme"));
    }
    std::optional<std::string> agent_value;
    if (identifier.has_value()) {
      POSTPROJECT_TRY_ASSIGN(
          agent_value,
          detail::checked_string(identifier->value, "agent identifier value"));
    }
    std::optional<std::string> agent_qualifier;
    if (identifier.has_value()) {
      POSTPROJECT_TRY_ASSIGN(
          agent_qualifier,
          detail::checked_optional_string(identifier->qualifier,
                                          "agent identifier qualifier"));
    }
    pp_uuid_t claim_id{};
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_transaction_claim_job(
        transaction_, &native_job_id, tool_name.c_str(),
        tool_version.has_value() ? tool_version->c_str() : nullptr,
        tool_uri.has_value() ? tool_uri->c_str() : nullptr,
        agent_name.has_value() ? agent_name->c_str() : nullptr,
        agent_scheme.has_value() ? agent_scheme->c_str() : nullptr,
        agent_value.has_value() ? agent_value->c_str() : nullptr,
        agent_qualifier.has_value() ? agent_qualifier->c_str() : nullptr,
        now_unix_micros, expires_at_unix_micros, &claim_id, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return detail::uuid(claim_id);
  }

  Result<void> renewJobClaim(const Uuid &job_id, const Uuid &claim_id,
                             std::int64_t now_unix_micros,
                             std::int64_t expires_at_unix_micros) {
    const pp_uuid_t native_job_id = detail::native_uuid(job_id);
    const pp_uuid_t native_claim_id = detail::native_uuid(claim_id);
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_transaction_renew_job_claim(
        transaction_, &native_job_id, &native_claim_id, now_unix_micros,
        expires_at_unix_micros, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return {};
  }

  Result<void> releaseJobClaim(const Uuid &job_id, const Uuid &claim_id) {
    const pp_uuid_t native_job_id = detail::native_uuid(job_id);
    const pp_uuid_t native_claim_id = detail::native_uuid(claim_id);
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_transaction_release_job_claim(
        transaction_, &native_job_id, &native_claim_id, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return {};
  }

  Result<void> completeJob(const Uuid &job_id, const Uuid &claim_id,
                           std::int64_t now_unix_micros,
                           const Uuid &output_representation_id,
                           const Uuid &activity_id) {
    const pp_uuid_t native_job_id = detail::native_uuid(job_id);
    const pp_uuid_t native_claim_id = detail::native_uuid(claim_id);
    const pp_uuid_t native_output_id =
        detail::native_uuid(output_representation_id);
    const pp_uuid_t native_activity_id = detail::native_uuid(activity_id);
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_transaction_complete_job(
        transaction_, &native_job_id, &native_claim_id, now_unix_micros,
        &native_output_id, &native_activity_id, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return {};
  }

  Result<void> failJob(const Uuid &job_id, const Uuid &claim_id,
                       std::int64_t now_unix_micros,
                       std::string_view diagnostic) {
    const pp_uuid_t native_job_id = detail::native_uuid(job_id);
    const pp_uuid_t native_claim_id = detail::native_uuid(claim_id);
    POSTPROJECT_TRY_ASSIGN(
        const std::string native_diagnostic,
        detail::checked_string(diagnostic, "job failure diagnostic"));
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_transaction_fail_job(
        transaction_, &native_job_id, &native_claim_id, now_unix_micros,
        native_diagnostic.c_str(), &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return {};
  }

  Result<void> cancelJob(const Uuid &job_id) {
    const pp_uuid_t native_job_id = detail::native_uuid(job_id);
    pp_error_t *error = nullptr;
    const pp_error_code_t status =
        pp_transaction_cancel_job(transaction_, &native_job_id, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return {};
  }

  Result<Uuid> createActivity(const ActivitySpec &spec) {
    POSTPROJECT_TRY_ASSIGN(const std::string kind,
                           detail::checked_string(spec.kind, "kind"));
    std::vector<pp_activity_edge_t> inputs;
    inputs.reserve(spec.inputs.size());
    for (const ActivityEdge &edge : spec.inputs) {
      if (edge.role.has_value()) {
        static_cast<void>(detail::checked_string(*edge.role, "input role"));
      }
      inputs.push_back(
          {detail::native_uuid(edge.representation_id),
           edge.role.has_value() ? edge.role->c_str() : nullptr});
    }
    std::vector<pp_activity_edge_t> outputs;
    outputs.reserve(spec.outputs.size());
    for (const ActivityEdge &edge : spec.outputs) {
      if (edge.role.has_value()) {
        static_cast<void>(detail::checked_string(*edge.role, "output role"));
      }
      outputs.push_back(
          {detail::native_uuid(edge.representation_id),
           edge.role.has_value() ? edge.role->c_str() : nullptr});
    }

    std::optional<std::string> tool_name;
    if (spec.tool.has_value()) {
      POSTPROJECT_TRY_ASSIGN(
          tool_name, detail::checked_string(spec.tool->name, "tool name"));
    }
    std::optional<std::string> tool_version;
    if (spec.tool.has_value()) {
      POSTPROJECT_TRY_ASSIGN(
          tool_version,
          detail::checked_optional_string(spec.tool->version, "tool version"));
    }
    std::optional<std::string> tool_uri;
    if (spec.tool.has_value()) {
      POSTPROJECT_TRY_ASSIGN(tool_uri, detail::checked_optional_string(
                                           spec.tool->uri, "tool URI"));
    }
    std::optional<std::string> agent_name;
    if (spec.agent.has_value()) {
      POSTPROJECT_TRY_ASSIGN(agent_name, detail::checked_optional_string(
                                             spec.agent->name, "agent name"));
    }
    const std::optional<ExternalIdentifier> identifier =
        spec.agent.has_value() ? spec.agent->identifier : std::nullopt;
    std::optional<std::string> agent_scheme;
    if (identifier.has_value()) {
      POSTPROJECT_TRY_ASSIGN(agent_scheme,
                             detail::checked_string(identifier->scheme,
                                                    "agent identifier scheme"));
    }
    std::optional<std::string> agent_value;
    if (identifier.has_value()) {
      POSTPROJECT_TRY_ASSIGN(
          agent_value,
          detail::checked_string(identifier->value, "agent identifier value"));
    }
    std::optional<std::string> agent_qualifier;
    if (identifier.has_value()) {
      POSTPROJECT_TRY_ASSIGN(
          agent_qualifier,
          detail::checked_optional_string(identifier->qualifier,
                                          "agent identifier qualifier"));
    }

    pp_uuid_t activity_id{};
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_transaction_create_activity(
        transaction_, kind.c_str(), inputs.data(),
        static_cast<std::uint64_t>(inputs.size()), outputs.data(),
        static_cast<std::uint64_t>(outputs.size()),
        spec.started_at_unix_micros.has_value()
            ? &*spec.started_at_unix_micros
            : nullptr,
        spec.finished_at_unix_micros.has_value()
            ? &*spec.finished_at_unix_micros
            : nullptr,
        tool_name.has_value() ? tool_name->c_str() : nullptr,
        tool_version.has_value() ? tool_version->c_str() : nullptr,
        tool_uri.has_value() ? tool_uri->c_str() : nullptr,
        agent_name.has_value() ? agent_name->c_str() : nullptr,
        agent_scheme.has_value() ? agent_scheme->c_str() : nullptr,
        agent_value.has_value() ? agent_value->c_str() : nullptr,
        agent_qualifier.has_value() ? agent_qualifier->c_str() : nullptr,
        &activity_id, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return detail::uuid(activity_id);
  }

  Result<void> commit() {
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_transaction_commit(transaction_, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return {};
  }

  [[nodiscard]] Result<CommitReceipt> commitWithReceipt() {
    pp_commit_receipt_t receipt{};
    pp_error_t *error = nullptr;
    const pp_error_code_t status =
        pp_transaction_commit_with_receipt(transaction_, &receipt, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    CommitReceipt result{detail::production_id(receipt.production_id), std::nullopt};
    if (receipt.outcome == PP_COMMIT_REVISION_CREATED) {
      result.revision = CommittedRevision{detail::uuid(receipt.revision_id),
                                         receipt.revision_sequence};
    }
    return result;
  }

  Result<void> rollback() {
    pp_error_t *error = nullptr;
    const pp_error_code_t status =
        pp_transaction_rollback(transaction_, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return {};
  }

  Transaction(const Transaction &) = delete;
  Transaction &operator=(const Transaction &) = delete;

  Transaction(Transaction &&other) noexcept
      : transaction_(std::exchange(other.transaction_, nullptr)) {}

  Transaction &operator=(Transaction &&other) noexcept {
    if (this != &other) {
      pp_transaction_release(transaction_);
      transaction_ = std::exchange(other.transaction_, nullptr);
    }
    return *this;
  }

  ~Transaction() { pp_transaction_release(transaction_); }

  [[nodiscard]] explicit operator bool() const noexcept {
    return transaction_ != nullptr;
  }

private:
  friend class Production;
  friend class ReadSession;

  explicit Transaction(pp_transaction_t *transaction) noexcept
      : transaction_(transaction) {}

  static Result<std::string> checked_origin_name(const OriginIdentity &origin) {
    return detail::checked_string(origin.name, "origin name");
  }

  Result<Uuid> import_media_impl(const MediaSource &source,
                                 const char *display_name) {
    POSTPROJECT_TRY_ASSIGN(const detail::MediaSourceHandle native_source,
                           source.native());
    pp_uuid_t value{};
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_transaction_import_media(
        transaction_, native_source.get(), display_name, &value, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return detail::uuid(value);
  }

  Result<Uuid> add_media_root_impl(std::string_view name, const char *label,
                                   std::int32_t priority) {
    POSTPROJECT_TRY_ASSIGN(const std::string native_name,
                           detail::checked_string(name, "name"));
    pp_uuid_t value{};
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_transaction_add_media_root(
        transaction_, native_name.c_str(), label, priority, &value, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return detail::uuid(value);
  }

  Result<void>
  mutate_external_identifier(bool remove, const ObjectRef &target,
                             const ExternalIdentifier &identifier) {
    const pp_object_ref_t native_target = detail::native_object_ref(target);
    POSTPROJECT_TRY_ASSIGN(const std::string scheme,
                           detail::checked_string(identifier.scheme, "scheme"));
    POSTPROJECT_TRY_ASSIGN(const std::string value,
                           detail::checked_string(identifier.value, "value"));
    std::optional<std::string> qualifier;
    if (identifier.qualifier.has_value()) {
      POSTPROJECT_TRY_ASSIGN(
          qualifier,
          detail::checked_string(*identifier.qualifier, "qualifier"));
    }
    pp_error_t *error = nullptr;
    const pp_error_code_t status = remove
                                       ? pp_transaction_remove_external_identifier(
                                             transaction_, &native_target,
                                             scheme.c_str(), value.c_str(),
                                             qualifier.has_value()
                                                 ? qualifier->c_str()
                                                 : nullptr,
                                             &error)
                                       : pp_transaction_add_external_identifier(
                                             transaction_, &native_target,
                                             scheme.c_str(), value.c_str(),
                                             qualifier.has_value()
                                                 ? qualifier->c_str()
                                                 : nullptr,
                                             &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return {};
  }

  pp_transaction_t *transaction_ = nullptr;
};

// Move-only owner of a thread-safe native handle. Concurrent const calls are
// supported while ownership operations and destruction remain serialized.
// A cancellation flag shared with running operations. cancel() may be called
// from any thread; an observing operation then returns ErrorCode::cancelled.
class CancelToken final {
public:
  [[nodiscard]] static Result<CancelToken> create() {
    pp_cancel_token_t *token = nullptr;
    pp_error_t *error = nullptr;
    const auto status = pp_cancel_token_create(&token, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return CancelToken(token);
  }
  CancelToken(const CancelToken &) = delete;
  CancelToken &operator=(const CancelToken &) = delete;
  CancelToken(CancelToken &&other) noexcept
      : token_(std::exchange(other.token_, nullptr)) {}
  CancelToken &operator=(CancelToken &&other) noexcept {
    if (this != &other) {
      pp_cancel_token_release(token_);
      token_ = std::exchange(other.token_, nullptr);
    }
    return *this;
  }
  ~CancelToken() { pp_cancel_token_release(token_); }

  void cancel() const noexcept { pp_cancel_token_cancel(token_); }

private:
  friend class ResolutionOptions;
  explicit CancelToken(pp_cancel_token_t *token) noexcept : token_(token) {}
  pp_cancel_token_t *token_ = nullptr;
};

// Validated machine-local resolution settings. Setters report errors immediately
// and preserve the previous settings on failure. Ownership remains move-only.
class ResolutionOptions final {
public:
  [[nodiscard]] static Result<ResolutionOptions> create() {
    pp_resolution_options_t *options = nullptr;
    pp_error_t *error = nullptr;
    const auto status = pp_resolution_options_create(&options, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return ResolutionOptions(options);
  }
  ResolutionOptions(const ResolutionOptions &) = delete;
  ResolutionOptions &operator=(const ResolutionOptions &) = delete;
  ResolutionOptions(ResolutionOptions &&other) noexcept
      : options_(std::exchange(other.options_, nullptr)) {}
  ResolutionOptions &operator=(ResolutionOptions &&other) noexcept {
    if (this != &other) {
      pp_resolution_options_release(options_);
      options_ = std::exchange(other.options_, nullptr);
    }
    return *this;
  }
  ~ResolutionOptions() { pp_resolution_options_release(options_); }

  [[nodiscard]] Result<void> addRootMapping(std::string_view name,
                                          std::string_view directory) {
    POSTPROJECT_TRY_ASSIGN(const auto native_name,
                           detail::checked_string(name, "root name"));
    POSTPROJECT_TRY_ASSIGN(const auto native_directory,
                           detail::checked_string(directory, "root directory"));
    pp_error_t *error = nullptr;
    const auto status = pp_resolution_options_add_root_mapping(
        options_, native_name.c_str(), native_directory.c_str(), &error);
    return detail::check(status, error);
  }

  [[nodiscard]] Result<void> addSearchDirectory(std::string_view directory) {
    POSTPROJECT_TRY_ASSIGN(const auto native_directory,
                           detail::checked_string(directory, "search directory"));
    pp_error_t *error = nullptr;
    const auto status = pp_resolution_options_add_search_directory(
        options_, native_directory.c_str(), &error);
    return detail::check(status, error);
  }

  [[nodiscard]] Result<void> setVerification(VerificationMode verification) {
    pp_error_t *error = nullptr;
    const auto status = pp_resolution_options_set_verification(
        options_, static_cast<pp_verification_mode_t>(verification), &error);
    return detail::check(status, error);
  }

  [[nodiscard]] Result<void> setLimits(std::uint32_t max_depth,
                                     std::uint64_t max_entries_per_directory) {
    pp_error_t *error = nullptr;
    const auto status = pp_resolution_options_set_limits(
        options_, max_depth, max_entries_per_directory, &error);
    return detail::check(status, error);
  }

  // The options retain the flag independently of the token's lifetime.
  [[nodiscard]] Result<void> setCancelToken(const CancelToken &token) {
    if (token.token_ == nullptr)
      return Error(ErrorCode::invalid_argument, "cancellation token was moved from");
    pp_error_t *error = nullptr;
    const auto status = pp_resolution_options_set_cancel_token(
        options_, token.token_, &error);
    return detail::check(status, error);
  }

private:
  friend class Production;
  friend class ReadSession;
  explicit ResolutionOptions(pp_resolution_options_t *options) noexcept
      : options_(options) {}
  pp_resolution_options_t *options_;
};

class ReadSession final {
public:
  [[nodiscard]] Result<ArtifactEvaluation>
  evaluateArtifact(const Uuid &representation_id, std::uint32_t max_depth = 64,
                   std::uint32_t max_representations = 1000) const {
    const pp_uuid_t native_id = detail::native_uuid(representation_id);
    pp_artifact_evaluation_t *raw_evaluation = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_read_session_evaluate_artifact(
        session_, &native_id, max_depth, max_representations,
        &raw_evaluation, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    detail::ArtifactEvaluationHandle evaluation(raw_evaluation);

    return detail::artifact_evaluation(std::move(evaluation));
  }

  [[nodiscard]] Result<ArtifactReproducibility>
  artifactReproducibility(const Uuid &representation_id) const {
    const pp_uuid_t native_id = detail::native_uuid(representation_id);
    pp_artifact_reproducibility_t *raw_report = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_read_session_artifact_reproducibility(
        session_, &native_id, &raw_report, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    detail::ArtifactReproducibilityHandle report(raw_report);

    return detail::artifact_reproducibility(std::move(report));
  }

  // Reads one job; an absent job is ErrorCode::not_found.
  [[nodiscard]] Result<Job> job(const Uuid &job_id) const {
    const pp_uuid_t native_id = detail::native_uuid(job_id);
    pp_job_set_t *raw_jobs = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status =
        pp_read_session_job(session_, &native_id, &raw_jobs, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    detail::JobSetHandle jobs(raw_jobs);
    if (pp_job_set_count(jobs.get()) != 1) {
      return Error(ErrorCode::internal, "job read returned no single job");
    }
    return detail::job(jobs.get(), 0);
  }

  [[nodiscard]] Result<QueryPage<Job>>
  jobs(std::uint32_t limit,
       std::optional<std::string_view> cursor = std::nullopt,
       std::optional<JobState> state = std::nullopt,
       std::optional<std::string_view> kind = std::nullopt) const {
    POSTPROJECT_TRY_ASSIGN(const std::optional<std::string> checked_cursor,
                           detail::checked_cursor(cursor));
    std::optional<std::string> checked_kind;
    if (kind.has_value()) {
      POSTPROJECT_TRY_ASSIGN(checked_kind,
                             detail::checked_string(*kind, "job kind"));
    }
    pp_job_set_t *raw_jobs = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_read_session_jobs(
        session_,
        state.has_value() ? static_cast<pp_job_state_t>(*state) : 0,
        checked_kind.has_value() ? checked_kind->c_str() : nullptr, limit,
        detail::optional_c_str(checked_cursor),
        &raw_jobs, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    detail::JobSetHandle jobs(raw_jobs);

    return detail::job_page(std::move(jobs));
  }

  // Compares current files with the resource fingerprints in this view. For
  // an image sequence, path is its directory and sequence_naming names its
  // files; empty means the naming recorded for that directory.
  [[nodiscard]] Result<ContentVerification> verifyResource(
      const Uuid &resource_id, std::string_view path,
      const std::optional<SequenceNaming> &sequence_naming =
          std::nullopt) const {
    const pp_uuid_t id = detail::native_uuid(resource_id);
    POSTPROJECT_TRY_ASSIGN(const std::string native_path,
                           detail::checked_string(path, "path"));
    POSTPROJECT_TRY_ASSIGN(detail::NativeNaming naming,
                           detail::NativeNaming::make(sequence_naming));
    pp_content_verification_t verification = 0;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_read_session_verify_resource(
        session_, &id, native_path.c_str(), naming.get(), &verification,
        &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return static_cast<ContentVerification>(verification);
  }

  // Resolves one asset with default options: known locators only.
  [[nodiscard]] Result<std::vector<RepresentationResolution>>
  resolveAsset(const Uuid &asset_id) const {
    return resolveAssets({asset_id}, nullptr);
  }

  [[nodiscard]] Result<std::vector<RepresentationResolution>>
  resolveAsset(const Uuid &asset_id, const ResolutionOptions &options) const {
    if (options.options_ == nullptr) {
      return Error(ErrorCode::invalid_argument, "resolution options were moved from");
    }
    return resolveAssets({asset_id}, options.options_);
  }

  [[nodiscard]] Result<std::vector<RepresentationResolution>>
  resolveAssets(const std::vector<Uuid> &asset_ids) const {
    return resolveAssets(asset_ids, nullptr);
  }

  // Resolves every representation of each asset, in asset order, scanning the
  // search scope once for the whole call.
  [[nodiscard]] Result<std::vector<RepresentationResolution>>
  resolveAssets(const std::vector<Uuid> &asset_ids,
                const ResolutionOptions &options) const {
    if (options.options_ == nullptr) {
      return Error(ErrorCode::invalid_argument, "resolution options were moved from");
    }
    return resolveAssets(asset_ids, options.options_);
  }

  // Assertions of one property, optionally restricted to an exact scalar value.
  [[nodiscard]] Result<QueryPage<MetadataAssertion>>
  queryMetadata(std::string_view vocabulary, std::string_view property,
                std::uint32_t limit,
                std::optional<std::string_view> cursor = std::nullopt) const {
    return query_metadata_impl(vocabulary, property, nullptr, limit, cursor);
  }

  [[nodiscard]] Result<QueryPage<MetadataAssertion>>
  queryMetadata(std::string_view vocabulary, std::string_view property,
                const MetadataValue &exact_value, std::uint32_t limit,
                std::optional<std::string_view> cursor = std::nullopt) const {
    POSTPROJECT_TRY_ASSIGN(const detail::MetadataInputHandle native_value,
                           detail::native_metadata_input(exact_value));
    return query_metadata_impl(vocabulary, property, native_value.get(), limit,
                               cursor);
  }

  // Representations that use a resource, in identity order.
  [[nodiscard]] Result<QueryPage<Representation>> representationsUsingResource(
      const Uuid &resource_id, std::uint32_t limit,
      std::optional<std::string_view> cursor = std::nullopt) const {
    const pp_uuid_t native_id = detail::native_uuid(resource_id);
    POSTPROJECT_TRY_ASSIGN(const std::optional<std::string> checked_cursor,
                           detail::checked_cursor(cursor));
    pp_representation_set_t *raw_representations = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status =
        pp_read_session_representations_using_resource(
            session_, &native_id, limit,
            detail::optional_c_str(checked_cursor), &raw_representations,
            &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return detail::representation_page(
        detail::RepresentationSetHandle(raw_representations));
  }

  // Resource identities of one representation in structure order.
  [[nodiscard]] Result<QueryPage<Uuid>>
  resources(const Uuid &representation_id, std::uint32_t limit,
            std::optional<std::string_view> cursor = std::nullopt) const {
    const pp_uuid_t native_id = detail::native_uuid(representation_id);
    POSTPROJECT_TRY_ASSIGN(const std::optional<std::string> checked_cursor,
                           detail::checked_cursor(cursor));
    pp_object_query_set_t *raw_objects = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_read_session_resources_page(
        session_, &native_id, limit, detail::optional_c_str(checked_cursor),
        &raw_objects, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    POSTPROJECT_TRY_ASSIGN(
        QueryPage<ObjectMatch> page,
        detail::object_query_page(detail::ObjectQuerySetHandle(raw_objects)));
    return detail::object_id_page(std::move(page), ObjectKind::resource);
  }

  [[nodiscard]] Result<QueryPage<ResourceLocator>>
  locators(const Uuid &resource_id, std::uint32_t limit,
           std::optional<std::string_view> cursor = std::nullopt) const {
    const pp_uuid_t native_id = detail::native_uuid(resource_id);
    POSTPROJECT_TRY_ASSIGN(const std::optional<std::string> checked_cursor,
                           detail::checked_cursor(cursor));
    pp_locator_query_set_t *raw_locators = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_read_session_locators_page(
        session_, &native_id, limit, detail::optional_c_str(checked_cursor),
        &raw_locators, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return detail::locator_page(detail::LocatorQuerySetHandle(raw_locators));
  }

  [[nodiscard]] Result<std::vector<MediaRoot>> mediaRoots() const {
    pp_media_root_set_t *raw_roots = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status =
        pp_read_session_media_roots(session_, &raw_roots, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    detail::MediaRootSetHandle roots(raw_roots);

    std::vector<MediaRoot> result;
    const std::uint64_t count = pp_media_root_set_count(roots.get());
    result.reserve(static_cast<std::size_t>(count));
    for (std::uint64_t index = 0; index < count; ++index) {
      pp_uuid_t id{};
      const char *name = nullptr;
      const char *label = nullptr;
      const char *legacy_uri = nullptr;
      std::int32_t priority = 0;
      std::uint8_t enabled = 0;
      pp_error_t *item_error = nullptr;
      const pp_error_code_t item_status = pp_media_root_set_get(
          roots.get(), index, &id, &name, &label, &legacy_uri, &priority,
          &enabled, &item_error);
      POSTPROJECT_TRY(detail::check(item_status, item_error));
      if (name == nullptr) {
        return Error(ErrorCode::internal, "media root has no name");
      }
      result.push_back(
          {detail::uuid(id), std::string(name),
           label != nullptr ? std::optional<std::string>(std::string(label))
                            : std::nullopt,
           legacy_uri != nullptr
               ? std::optional<std::string>(std::string(legacy_uri))
               : std::nullopt,
           priority, enabled != 0});
    }
    return result;
  }

  [[nodiscard]] Result<std::vector<ExternalIdentifier>>
  externalIdentifiers(const ObjectRef &target) const {
    const pp_object_ref_t native_target = detail::native_object_ref(target);
    pp_external_identifier_set_t *raw_identifiers = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_read_session_external_identifiers(
        session_, &native_target, &raw_identifiers, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    detail::ExternalIdentifierSetHandle identifiers(raw_identifiers);

    std::vector<ExternalIdentifier> result;
    const std::uint64_t count =
        pp_external_identifier_set_count(identifiers.get());
    result.reserve(static_cast<std::size_t>(count));
    for (std::uint64_t index = 0; index < count; ++index) {
      const char *scheme = nullptr;
      const char *value = nullptr;
      const char *qualifier = nullptr;
      pp_error_t *item_error = nullptr;
      const pp_error_code_t item_status = pp_external_identifier_set_get(
          identifiers.get(), index, &scheme, &value, &qualifier, &item_error);
      POSTPROJECT_TRY(detail::check(item_status, item_error));
      result.push_back(
          {scheme != nullptr ? std::string(scheme) : std::string(),
           value != nullptr ? std::string(value) : std::string(),
           qualifier != nullptr
               ? std::optional<std::string>(std::string(qualifier))
               : std::nullopt});
    }
    return result;
  }

  [[nodiscard]] Result<std::vector<ObjectRef>> findByExternalIdentifier(
      std::string_view scheme, std::string_view value,
      std::optional<std::string_view> qualifier = std::nullopt) const {
    POSTPROJECT_TRY_ASSIGN(const std::string native_scheme,
                           detail::checked_string(scheme, "scheme"));
    POSTPROJECT_TRY_ASSIGN(const std::string native_value,
                           detail::checked_string(value, "value"));
    std::optional<std::string> native_qualifier;
    if (qualifier.has_value()) {
      POSTPROJECT_TRY_ASSIGN(native_qualifier,
                             detail::checked_string(*qualifier, "qualifier"));
    }
    pp_object_ref_set_t *raw_objects = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_read_session_find_by_external_identifier(
        session_, native_scheme.c_str(), native_value.c_str(),
        native_qualifier.has_value() ? native_qualifier->c_str() : nullptr,
        &raw_objects, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    detail::ObjectRefSetHandle objects(raw_objects);

    std::vector<ObjectRef> result;
    const std::uint64_t count = pp_object_ref_set_count(objects.get());
    result.reserve(static_cast<std::size_t>(count));
    for (std::uint64_t index = 0; index < count; ++index) {
      pp_object_ref_t object{};
      pp_error_t *item_error = nullptr;
      const pp_error_code_t item_status =
          pp_object_ref_set_get(objects.get(), index, &object, &item_error);
      POSTPROJECT_TRY(detail::check(item_status, item_error));
      result.push_back(detail::object_ref(object));
    }
    return result;
  }

  [[nodiscard]] Result<QueryPage<KnownMediaMatch>> findKnownMediaByLocator(
      const LocatorIdentity &locator, std::uint32_t limit,
      std::optional<std::string_view> cursor = std::nullopt) const {
    POSTPROJECT_TRY_ASSIGN(const std::string uri,
                           detail::checked_string(locator.uri, "locator URI"));
    POSTPROJECT_TRY_ASSIGN(
        detail::NativeNaming naming,
        detail::NativeNaming::make(locator.sequence_naming));
    POSTPROJECT_TRY_ASSIGN(const std::optional<std::string> checked_cursor,
                           detail::checked_cursor(cursor));
    pp_known_media_set_t *raw_matches = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_read_session_find_known_media_by_locator(
        session_, uri.c_str(), naming.get(), limit,
        detail::optional_c_str(checked_cursor), &raw_matches, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return detail::known_media_page(
        detail::KnownMediaSetHandle(raw_matches));
  }

  [[nodiscard]] Result<QueryPage<KnownMediaMatch>> findKnownMediaByFingerprint(
      const Fingerprint &fingerprint, std::uint32_t limit,
      std::optional<std::string_view> cursor = std::nullopt) const {
    POSTPROJECT_TRY_ASSIGN(
        const std::string algorithm,
        detail::checked_string(fingerprint.algorithm, "fingerprint algorithm"));
    POSTPROJECT_TRY_ASSIGN(const std::optional<std::string> checked_cursor,
                           detail::checked_cursor(cursor));
    pp_known_media_set_t *raw_matches = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status =
        pp_read_session_find_known_media_by_fingerprint(
            session_, algorithm.c_str(), fingerprint.version,
            fingerprint.value.data(),
            static_cast<std::uint64_t>(fingerprint.value.size()), limit,
            detail::optional_c_str(checked_cursor), &raw_matches, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return detail::known_media_page(
        detail::KnownMediaSetHandle(raw_matches));
  }

  ReadSession(const ReadSession &) = delete;
  ReadSession &operator=(const ReadSession &) = delete;
  ReadSession(ReadSession &&other) noexcept : session_(std::exchange(other.session_, nullptr)) {}
  ReadSession &operator=(ReadSession &&other) noexcept {
    if (this != &other) { pp_read_session_release(session_); session_ = std::exchange(other.session_, nullptr); }
    return *this;
  }
  ~ReadSession() { pp_read_session_release(session_); }

  [[nodiscard]] Result<DecisionBase> decisionBase() const {
    pp_decision_base_t base{};
    pp_error_t *error = nullptr;
    const auto status = pp_read_session_decision_base(session_, &base, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return detail::decision_base(base);
  }
  [[nodiscard]] Result<Transaction> edit() const {
    pp_transaction_t *transaction = nullptr;
    pp_error_t *error = nullptr;
    const auto status = pp_read_session_begin_edit(session_, &transaction, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return Transaction(transaction);
  }
  [[nodiscard]] Result<QueryPage<Asset>>
  assets(std::uint32_t limit,
         std::optional<std::string_view> cursor = std::nullopt) const {
    POSTPROJECT_TRY_ASSIGN(const std::optional<std::string> checked_cursor,
                           detail::checked_cursor(cursor));
    pp_asset_set_t *raw_assets = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_read_session_assets_page(
        session_, limit, detail::optional_c_str(checked_cursor), &raw_assets,
        &error);
    POSTPROJECT_TRY(detail::check(status, error));
    detail::AssetSetHandle assets(raw_assets);

    std::vector<Asset> items;
    const std::uint64_t count = pp_asset_set_count(assets.get());
    items.reserve(static_cast<std::size_t>(count));
    for (std::uint64_t index = 0; index < count; ++index) {
      POSTPROJECT_TRY_ASSIGN(auto item_9, detail::asset(assets.get(), index));
      items.push_back(std::move(item_9));
    }
    const char *next_cursor = pp_asset_set_next_cursor(assets.get());
    return QueryPage<Asset>{std::move(items),
                            detail::optional_string(next_cursor), false};
  }

  [[nodiscard]] Result<Asset> asset(const Uuid &asset_id) const {
    const pp_uuid_t native_asset_id = detail::native_uuid(asset_id);
    pp_asset_set_t *raw_assets = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status =
        pp_read_session_asset(session_, &native_asset_id, &raw_assets, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    detail::AssetSetHandle assets(raw_assets);
    if (pp_asset_set_count(assets.get()) != 1) {
      return Error(ErrorCode::internal, "asset read returned no single asset");
    }
    return detail::asset(assets.get(), 0);
  }

  [[nodiscard]] Result<QueryPage<Representation>>
  representations(const Uuid &asset_id, std::uint32_t limit,
                  std::optional<std::string_view> cursor = std::nullopt) const {
    const pp_uuid_t native_asset_id = detail::native_uuid(asset_id);
    POSTPROJECT_TRY_ASSIGN(const std::optional<std::string> checked_cursor,
                           detail::checked_cursor(cursor));
    pp_representation_set_t *raw_representations = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_read_session_representations_page(
        session_, &native_asset_id, limit,
        detail::optional_c_str(checked_cursor), &raw_representations, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return detail::representation_page(
        detail::RepresentationSetHandle(raw_representations));
  }

  [[nodiscard]] Result<Representation>
  representation(const Uuid &representation_id) const {
    const pp_uuid_t native_id = detail::native_uuid(representation_id);
    pp_representation_set_t *raw_representations = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_read_session_representation(
        session_, &native_id, &raw_representations, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    detail::RepresentationSetHandle representations(raw_representations);
    if (pp_representation_set_count(representations.get()) != 1) {
      return Error(ErrorCode::internal,
                   "representation read returned no single representation");
    }
    return detail::representation(representations.get(), 0);
  }
private:
  [[nodiscard]] Result<std::vector<RepresentationResolution>>
  resolveAssets(const std::vector<Uuid> &asset_ids,
                const pp_resolution_options_t *options) const {
    std::vector<pp_uuid_t> native_ids;
    native_ids.reserve(asset_ids.size());
    for (const Uuid &asset_id : asset_ids) {
      native_ids.push_back(detail::native_uuid(asset_id));
    }
    pp_resolution_set_t *raw_resolutions = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_read_session_resolve_assets(
        session_, native_ids.empty() ? nullptr : native_ids.data(),
        static_cast<std::uint64_t>(native_ids.size()), options,
        &raw_resolutions, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    detail::ResolutionSetHandle resolutions(raw_resolutions);

    return detail::resolution_values(std::move(resolutions));
  }

  [[nodiscard]] Result<QueryPage<MetadataAssertion>>
  query_metadata_impl(std::string_view vocabulary, std::string_view property,
                      const pp_metadata_input_t *exact_value,
                      std::uint32_t limit,
                      const std::optional<std::string_view> &cursor) const {
    POSTPROJECT_TRY_ASSIGN(
        const std::string native_vocabulary,
        detail::checked_string(vocabulary, "metadata vocabulary"));
    POSTPROJECT_TRY_ASSIGN(
        const std::string native_property,
        detail::checked_string(property, "metadata property"));
    POSTPROJECT_TRY_ASSIGN(const std::optional<std::string> checked_cursor,
                           detail::checked_cursor(cursor));
    pp_metadata_set_t *raw_metadata = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_read_session_query_metadata(
        session_, native_vocabulary.c_str(), native_property.c_str(),
        exact_value, limit, detail::optional_c_str(checked_cursor),
        &raw_metadata, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return detail::metadata_page(detail::MetadataSetHandle(raw_metadata));
  }

  friend class Production;
  explicit ReadSession(pp_read_session_t *session) noexcept : session_(session) {}
  pp_read_session_t *session_ = nullptr;
};

class Production final {
public:
  static Result<Production> create(std::string_view path) {
    return create_impl(path, nullptr);
  }

  static Result<Production> create(std::string_view path,
                                   std::string_view display_name) {
    POSTPROJECT_TRY_ASSIGN(
        const std::string name,
        detail::checked_string(display_name, "display_name"));
    return create_impl(path, name.c_str());
  }

  static Result<Production> open(std::string_view path) {
    POSTPROJECT_TRY_ASSIGN(const std::string native_path,
                           detail::checked_string(path, "path"));
    pp_production_t *production = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status =
        pp_production_open(native_path.c_str(), &production, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return Production(production);
  }

  Production(const Production &) = delete;
  Production &operator=(const Production &) = delete;

  Production(Production &&other) noexcept
      : production_(std::exchange(other.production_, nullptr)) {}

  Production &operator=(Production &&other) noexcept {
    if (this != &other) {
      pp_production_release(production_);
      production_ = std::exchange(other.production_, nullptr);
    }
    return *this;
  }

  ~Production() { pp_production_release(production_); }

  [[nodiscard]] Result<ReadSession> readSession() const {
    pp_read_session_t *session = nullptr;
    pp_error_t *error = nullptr;
    const auto status = pp_production_read_session(production_, &session, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return ReadSession(session);
  }

  [[nodiscard]] Result<Transaction> edit(const DecisionBase &base) const {
    const auto native = detail::native_decision_base(base);
    pp_transaction_t *transaction = nullptr;
    pp_error_t *error = nullptr;
    const auto status = pp_production_begin_edit(production_, &native, &transaction, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return Transaction(transaction);
  }

  [[nodiscard]] Result<ProductionId> id() const {
    pp_production_id_t value{};
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_production_id(production_, &value, &error);
    POSTPROJECT_TRY(detail::check(status, error));

    return detail::production_id(value);
  }

  [[nodiscard]] Result<bool> containsAsset(const Uuid &asset_id) const {
    const pp_uuid_t value = detail::native_uuid(asset_id);
    std::uint8_t exists = 0;
    pp_error_t *error = nullptr;
    const pp_error_code_t status =
        pp_production_asset_exists(production_, &value, &exists, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return exists != 0;
  }

  [[nodiscard]] Result<std::vector<Asset>> assets() const {
    pp_asset_set_t *raw_assets = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status =
        pp_production_assets(production_, &raw_assets, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    detail::AssetSetHandle assets(raw_assets);

    std::vector<Asset> result;
    const std::uint64_t count = pp_asset_set_count(assets.get());
    result.reserve(static_cast<std::size_t>(count));
    for (std::uint64_t index = 0; index < count; ++index) {
      POSTPROJECT_TRY_ASSIGN(auto item_8, detail::asset(assets.get(), index));
      result.push_back(std::move(item_8));
    }
    return result;
  }

  [[nodiscard]] Result<QueryPage<Asset>>
  assets(std::uint32_t limit,
         std::optional<std::string_view> cursor = std::nullopt) const {
    POSTPROJECT_TRY_ASSIGN(const std::optional<std::string> checked_cursor,
                           detail::checked_cursor(cursor));
    pp_asset_set_t *raw_assets = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_production_assets_page(
        production_, limit, detail::optional_c_str(checked_cursor), &raw_assets,
        &error);
    POSTPROJECT_TRY(detail::check(status, error));
    detail::AssetSetHandle assets(raw_assets);

    std::vector<Asset> items;
    const std::uint64_t count = pp_asset_set_count(assets.get());
    items.reserve(static_cast<std::size_t>(count));
    for (std::uint64_t index = 0; index < count; ++index) {
      POSTPROJECT_TRY_ASSIGN(auto item_9, detail::asset(assets.get(), index));
      items.push_back(std::move(item_9));
    }
    const char *next_cursor = pp_asset_set_next_cursor(assets.get());
    return QueryPage<Asset>{std::move(items),
                            detail::optional_string(next_cursor), false};
  }

  // Reads one asset; an absent asset is ErrorCode::not_found.
  [[nodiscard]] Result<Asset> asset(const Uuid &asset_id) const {
    const pp_uuid_t native_asset_id = detail::native_uuid(asset_id);
    pp_asset_set_t *raw_assets = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status =
        pp_production_asset(production_, &native_asset_id, &raw_assets, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    detail::AssetSetHandle assets(raw_assets);
    if (pp_asset_set_count(assets.get()) != 1) {
      return Error(ErrorCode::internal, "asset read returned no single asset");
    }
    return detail::asset(assets.get(), 0);
  }

  [[nodiscard]] Result<std::vector<MediaRoot>> mediaRoots() const {
    pp_media_root_set_t *raw_roots = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status =
        pp_production_media_roots(production_, &raw_roots, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    detail::MediaRootSetHandle roots(raw_roots);

    std::vector<MediaRoot> result;
    const std::uint64_t count = pp_media_root_set_count(roots.get());
    result.reserve(static_cast<std::size_t>(count));
    for (std::uint64_t index = 0; index < count; ++index) {
      pp_uuid_t id{};
      const char *name = nullptr;
      const char *label = nullptr;
      const char *legacy_uri = nullptr;
      std::int32_t priority = 0;
      std::uint8_t enabled = 0;
      pp_error_t *item_error = nullptr;
      const pp_error_code_t item_status = pp_media_root_set_get(
          roots.get(), index, &id, &name, &label, &legacy_uri, &priority,
          &enabled, &item_error);
      POSTPROJECT_TRY(detail::check(item_status, item_error));
      if (name == nullptr) {
        return Error(ErrorCode::internal, "media root has no name");
      }
      result.push_back(
          {detail::uuid(id), std::string(name),
           label != nullptr ? std::optional<std::string>(std::string(label))
                            : std::nullopt,
           legacy_uri != nullptr
               ? std::optional<std::string>(std::string(legacy_uri))
               : std::nullopt,
           priority, enabled != 0});
    }
    return result;
  }

  [[nodiscard]] Result<std::vector<Representation>>
  representations(const Uuid &asset_id) const {
    const pp_uuid_t native_asset_id = detail::native_uuid(asset_id);
    pp_representation_set_t *raw_representations = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_production_representations(
        production_, &native_asset_id, &raw_representations, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    detail::RepresentationSetHandle representations(raw_representations);

    std::vector<Representation> result;
    const std::uint64_t count =
        pp_representation_set_count(representations.get());
    result.reserve(static_cast<std::size_t>(count));
    for (std::uint64_t index = 0; index < count; ++index) {
      POSTPROJECT_TRY_ASSIGN(
          auto item_10, detail::representation(representations.get(), index));
      result.push_back(std::move(item_10));
    }
    return result;
  }

  [[nodiscard]] Result<QueryPage<Representation>>
  representations(const Uuid &asset_id, std::uint32_t limit,
                  std::optional<std::string_view> cursor = std::nullopt) const {
    const pp_uuid_t native_asset_id = detail::native_uuid(asset_id);
    POSTPROJECT_TRY_ASSIGN(const std::optional<std::string> checked_cursor,
                           detail::checked_cursor(cursor));
    pp_representation_set_t *raw_representations = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_production_representations_page(
        production_, &native_asset_id, limit,
        detail::optional_c_str(checked_cursor), &raw_representations, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return detail::representation_page(
        detail::RepresentationSetHandle(raw_representations));
  }

  // Reads one representation; an absent one is ErrorCode::not_found.
  [[nodiscard]] Result<Representation>
  representation(const Uuid &representation_id) const {
    const pp_uuid_t native_id = detail::native_uuid(representation_id);
    pp_representation_set_t *raw_representations = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_production_representation(
        production_, &native_id, &raw_representations, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    detail::RepresentationSetHandle representations(raw_representations);
    if (pp_representation_set_count(representations.get()) != 1) {
      return Error(ErrorCode::internal,
                   "representation read returned no single representation");
    }
    return detail::representation(representations.get(), 0);
  }

  // Representations that use a resource, in identity order.
  [[nodiscard]] Result<QueryPage<Representation>> representationsUsingResource(
      const Uuid &resource_id, std::uint32_t limit,
      std::optional<std::string_view> cursor = std::nullopt) const {
    const pp_uuid_t native_id = detail::native_uuid(resource_id);
    POSTPROJECT_TRY_ASSIGN(const std::optional<std::string> checked_cursor,
                           detail::checked_cursor(cursor));
    pp_representation_set_t *raw_representations = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status =
        pp_production_representations_using_resource(
            production_, &native_id, limit,
            detail::optional_c_str(checked_cursor), &raw_representations,
            &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return detail::representation_page(
        detail::RepresentationSetHandle(raw_representations));
  }

  // Representations with a locator confirmed under the named logical root.
  [[nodiscard]] Result<QueryPage<Representation>> representationsUnderMediaRoot(
      std::string_view root_name, std::uint32_t limit,
      std::optional<std::string_view> cursor = std::nullopt) const {
    POSTPROJECT_TRY_ASSIGN(const std::string native_root,
                           detail::checked_string(root_name, "root name"));
    POSTPROJECT_TRY_ASSIGN(const std::optional<std::string> checked_cursor,
                           detail::checked_cursor(cursor));
    pp_representation_set_t *raw_representations = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status =
        pp_production_representations_under_media_root(
            production_, native_root.c_str(), limit,
            detail::optional_c_str(checked_cursor), &raw_representations,
            &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return detail::representation_page(
        detail::RepresentationSetHandle(raw_representations));
  }

  // Resource identities of one representation in structure order.
  [[nodiscard]] Result<QueryPage<Uuid>>
  resources(const Uuid &representation_id, std::uint32_t limit,
            std::optional<std::string_view> cursor = std::nullopt) const {
    const pp_uuid_t native_id = detail::native_uuid(representation_id);
    POSTPROJECT_TRY_ASSIGN(const std::optional<std::string> checked_cursor,
                           detail::checked_cursor(cursor));
    pp_object_query_set_t *raw_objects = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_production_resources_page(
        production_, &native_id, limit, detail::optional_c_str(checked_cursor),
        &raw_objects, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    POSTPROJECT_TRY_ASSIGN(
        QueryPage<ObjectMatch> page,
        detail::object_query_page(detail::ObjectQuerySetHandle(raw_objects)));
    return detail::object_id_page(std::move(page), ObjectKind::resource);
  }

  [[nodiscard]] Result<QueryPage<ResourceLocator>>
  locators(const Uuid &resource_id, std::uint32_t limit,
           std::optional<std::string_view> cursor = std::nullopt) const {
    const pp_uuid_t native_id = detail::native_uuid(resource_id);
    POSTPROJECT_TRY_ASSIGN(const std::optional<std::string> checked_cursor,
                           detail::checked_cursor(cursor));
    pp_locator_query_set_t *raw_locators = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_production_locators_page(
        production_, &native_id, limit, detail::optional_c_str(checked_cursor),
        &raw_locators, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return detail::locator_page(detail::LocatorQuerySetHandle(raw_locators));
  }

  // Finds every current resource ownership candidate at this exact canonical
  // locator identity. The query is read-only and does not adopt media.
  [[nodiscard]] Result<QueryPage<KnownMediaMatch>> findKnownMediaByLocator(
      const LocatorIdentity &locator, std::uint32_t limit,
      std::optional<std::string_view> cursor = std::nullopt) const {
    POSTPROJECT_TRY_ASSIGN(const std::string uri,
                           detail::checked_string(locator.uri, "locator URI"));
    POSTPROJECT_TRY_ASSIGN(
        detail::NativeNaming naming,
        detail::NativeNaming::make(locator.sequence_naming));
    POSTPROJECT_TRY_ASSIGN(const std::optional<std::string> checked_cursor,
                           detail::checked_cursor(cursor));
    pp_known_media_set_t *raw_matches = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_production_find_known_media_by_locator(
        production_, uri.c_str(), naming.get(), limit,
        detail::optional_c_str(checked_cursor), &raw_matches, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return detail::known_media_page(
        detail::KnownMediaSetHandle(raw_matches));
  }

  // Finds every resource whose current effective fingerprint exactly matches.
  // Content equality produces candidates; it does not prove logical identity.
  [[nodiscard]] Result<QueryPage<KnownMediaMatch>> findKnownMediaByFingerprint(
      const Fingerprint &fingerprint, std::uint32_t limit,
      std::optional<std::string_view> cursor = std::nullopt) const {
    POSTPROJECT_TRY_ASSIGN(
        const std::string algorithm,
        detail::checked_string(fingerprint.algorithm, "fingerprint algorithm"));
    POSTPROJECT_TRY_ASSIGN(const std::optional<std::string> checked_cursor,
                           detail::checked_cursor(cursor));
    pp_known_media_set_t *raw_matches = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status =
        pp_production_find_known_media_by_fingerprint(
            production_, algorithm.c_str(), fingerprint.version,
            fingerprint.value.data(),
            static_cast<std::uint64_t>(fingerprint.value.size()), limit,
            detail::optional_c_str(checked_cursor), &raw_matches, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return detail::known_media_page(
        detail::KnownMediaSetHandle(raw_matches));
  }

  // Representations with a required resource that has no locator knowledge.
  [[nodiscard]] Result<QueryPage<Uuid>>
  unresolvedMedia(std::uint32_t limit,
                  std::optional<std::string_view> cursor = std::nullopt) const {
    POSTPROJECT_TRY_ASSIGN(const std::optional<std::string> checked_cursor,
                           detail::checked_cursor(cursor));
    pp_object_query_set_t *raw_objects = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_production_unresolved_media(
        production_, limit, detail::optional_c_str(checked_cursor),
        &raw_objects, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    POSTPROJECT_TRY_ASSIGN(
        QueryPage<ObjectMatch> page,
        detail::object_query_page(detail::ObjectQuerySetHandle(raw_objects)));
    return detail::object_id_page(std::move(page), ObjectKind::representation);
  }

  [[nodiscard]] Result<std::optional<DependencySet>>
  dependencySet(const Uuid &representation_id) const {
    const pp_uuid_t native_id = detail::native_uuid(representation_id);
    pp_dependency_set_t *raw_dependencies = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_production_dependency_set(
        production_, &native_id, &raw_dependencies, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    detail::DependencySetHandle dependencies(raw_dependencies);

    std::uint8_t present = 0;
    pp_uuid_t source_id{};
    std::uint64_t recorded_at_revision = 0;
    pp_dependency_set_status_t set_status = 0;
    std::uint64_t count = 0;
    pp_error_t *summary_error = nullptr;
    const pp_error_code_t summary_status = pp_dependency_set_get(
        dependencies.get(), &present, &source_id, &recorded_at_revision,
        &set_status, &count, &summary_error);
    POSTPROJECT_TRY(detail::check(summary_status, summary_error));
    if (present == 0) {
      return std::nullopt;
    }

    std::vector<Dependency> result;
    result.reserve(static_cast<std::size_t>(count));
    for (std::uint64_t index = 0; index < count; ++index) {
      pp_dependency_t native{};
      pp_error_t *item_error = nullptr;
      const pp_error_code_t item_status = pp_dependency_set_get_dependency(
          dependencies.get(), index, &native, &item_error);
      POSTPROJECT_TRY(detail::check(item_status, item_error));
      if (native.kind == nullptr || native.authored_reference == nullptr) {
        return Error(ErrorCode::internal, "dependency has a null string");
      }
      result.push_back(
          {native.has_source_resource != 0
               ? std::optional<Uuid>(detail::uuid(native.source_resource_id))
               : std::nullopt,
           std::string(native.kind), detail::object_ref(native.target),
           native.has_resolved_representation != 0
               ? std::optional<Uuid>(
                     detail::uuid(native.resolved_representation_id))
               : std::nullopt,
           native.required != 0, std::string(native.authored_reference)});
    }
    return DependencySet{detail::uuid(source_id), recorded_at_revision,
                         static_cast<DependencySetStatus>(set_status),
                         std::move(result)};
  }

  [[nodiscard]] Result<QueryPage<DependencyMatch>>
  dependencies(const Uuid &representation_id, std::uint32_t max_depth,
               std::uint32_t max_representations, std::uint32_t limit,
               std::optional<std::string_view> cursor = std::nullopt) const {
    const pp_uuid_t native_id = detail::native_uuid(representation_id);
    POSTPROJECT_TRY_ASSIGN(const std::optional<std::string> checked_cursor,
                           detail::checked_cursor(cursor));
    pp_dependency_query_set_t *raw_matches = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_production_dependencies(
        production_, &native_id, max_depth, max_representations, limit,
        detail::optional_c_str(checked_cursor),
        &raw_matches, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return detail::dependency_query_page(
        detail::DependencyQuerySetHandle(raw_matches));
  }

  [[nodiscard]] Result<QueryPage<DependencyMatch>>
  dependents(const ObjectRef &target, std::uint32_t max_depth,
             std::uint32_t max_representations, std::uint32_t limit,
             std::optional<std::string_view> cursor = std::nullopt) const {
    const pp_object_ref_t native_target = detail::native_object_ref(target);
    POSTPROJECT_TRY_ASSIGN(const std::optional<std::string> checked_cursor,
                           detail::checked_cursor(cursor));
    pp_dependency_query_set_t *raw_matches = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_production_dependents(
        production_, &native_target, max_depth, max_representations, limit,
        detail::optional_c_str(checked_cursor),
        &raw_matches, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return detail::dependency_query_page(
        detail::DependencyQuerySetHandle(raw_matches));
  }

  [[nodiscard]] Result<std::vector<ExternalIdentifier>>
  externalIdentifiers(const ObjectRef &target) const {
    const pp_object_ref_t native_target = detail::native_object_ref(target);
    pp_external_identifier_set_t *raw_identifiers = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_production_external_identifiers(
        production_, &native_target, &raw_identifiers, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    detail::ExternalIdentifierSetHandle identifiers(raw_identifiers);

    std::vector<ExternalIdentifier> result;
    const std::uint64_t count =
        pp_external_identifier_set_count(identifiers.get());
    result.reserve(static_cast<std::size_t>(count));
    for (std::uint64_t index = 0; index < count; ++index) {
      const char *scheme = nullptr;
      const char *value = nullptr;
      const char *qualifier = nullptr;
      pp_error_t *item_error = nullptr;
      const pp_error_code_t item_status = pp_external_identifier_set_get(
          identifiers.get(), index, &scheme, &value, &qualifier, &item_error);
      POSTPROJECT_TRY(detail::check(item_status, item_error));
      result.push_back(
          {scheme != nullptr ? std::string(scheme) : std::string(),
           value != nullptr ? std::string(value) : std::string(),
           qualifier != nullptr
               ? std::optional<std::string>(std::string(qualifier))
               : std::nullopt});
    }
    return result;
  }

  // A qualifier restricts matches to identifiers with exactly that qualifier.
  [[nodiscard]] Result<std::vector<ObjectRef>> findByExternalIdentifier(
      std::string_view scheme, std::string_view value,
      std::optional<std::string_view> qualifier = std::nullopt) const {
    POSTPROJECT_TRY_ASSIGN(const std::string native_scheme,
                           detail::checked_string(scheme, "scheme"));
    POSTPROJECT_TRY_ASSIGN(const std::string native_value,
                           detail::checked_string(value, "value"));
    std::optional<std::string> native_qualifier;
    if (qualifier.has_value()) {
      POSTPROJECT_TRY_ASSIGN(native_qualifier,
                             detail::checked_string(*qualifier, "qualifier"));
    }
    pp_object_ref_set_t *raw_objects = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_production_find_by_external_identifier(
        production_, native_scheme.c_str(), native_value.c_str(),
        native_qualifier.has_value() ? native_qualifier->c_str() : nullptr,
        &raw_objects, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    detail::ObjectRefSetHandle objects(raw_objects);

    std::vector<ObjectRef> result;
    const std::uint64_t count = pp_object_ref_set_count(objects.get());
    result.reserve(static_cast<std::size_t>(count));
    for (std::uint64_t index = 0; index < count; ++index) {
      pp_object_ref_t object{};
      pp_error_t *item_error = nullptr;
      const pp_error_code_t item_status =
          pp_object_ref_set_get(objects.get(), index, &object, &item_error);
      POSTPROJECT_TRY(detail::check(item_status, item_error));
      result.push_back(detail::object_ref(object));
    }
    return result;
  }

  // Assertions of one property, optionally restricted to an exact scalar value.
  [[nodiscard]] Result<QueryPage<MetadataAssertion>>
  queryMetadata(std::string_view vocabulary, std::string_view property,
                std::uint32_t limit,
                std::optional<std::string_view> cursor = std::nullopt) const {
    return query_metadata_impl(vocabulary, property, nullptr, limit, cursor);
  }

  [[nodiscard]] Result<QueryPage<MetadataAssertion>>
  queryMetadata(std::string_view vocabulary, std::string_view property,
                const MetadataValue &exact_value, std::uint32_t limit,
                std::optional<std::string_view> cursor = std::nullopt) const {
    POSTPROJECT_TRY_ASSIGN(const detail::MetadataInputHandle native_value,
                           detail::native_metadata_input(exact_value));
    return query_metadata_impl(vocabulary, property, native_value.get(), limit,
                               cursor);
  }

  // Compares the content at path with the resource's stored fingerprints. For
  // an image sequence, path is its directory and sequence_naming names its
  // files; empty means the naming recorded for that directory.
  [[nodiscard]] Result<ContentVerification> verifyResource(
      const Uuid &resource_id, std::string_view path,
      const std::optional<SequenceNaming> &sequence_naming =
          std::nullopt) const {
    const pp_uuid_t id = detail::native_uuid(resource_id);
    POSTPROJECT_TRY_ASSIGN(const std::string native_path,
                           detail::checked_string(path, "path"));
    POSTPROJECT_TRY_ASSIGN(detail::NativeNaming naming,
                           detail::NativeNaming::make(sequence_naming));
    pp_content_verification_t verification = 0;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_production_verify_resource(
        production_, &id, native_path.c_str(), naming.get(), &verification,
        &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return static_cast<ContentVerification>(verification);
  }

  // Resolves one asset with default options: known locators only.
  [[nodiscard]] Result<std::vector<RepresentationResolution>>
  resolveAsset(const Uuid &asset_id) const {
    return resolveAssets({asset_id}, nullptr);
  }

  [[nodiscard]] Result<std::vector<RepresentationResolution>>
  resolveAsset(const Uuid &asset_id, const ResolutionOptions &options) const {
    if (options.options_ == nullptr) {
      return Error(ErrorCode::invalid_argument, "resolution options were moved from");
    }
    return resolveAssets({asset_id}, options.options_);
  }

  [[nodiscard]] Result<std::vector<RepresentationResolution>>
  resolveAssets(const std::vector<Uuid> &asset_ids) const {
    return resolveAssets(asset_ids, nullptr);
  }

  // Resolves every representation of each asset, in asset order, scanning the
  // search scope once for the whole call.
  [[nodiscard]] Result<std::vector<RepresentationResolution>>
  resolveAssets(const std::vector<Uuid> &asset_ids,
                const ResolutionOptions &options) const {
    if (options.options_ == nullptr) {
      return Error(ErrorCode::invalid_argument, "resolution options were moved from");
    }
    return resolveAssets(asset_ids, options.options_);
  }

private:
  [[nodiscard]] Result<std::vector<RepresentationResolution>>
  resolveAssets(const std::vector<Uuid> &asset_ids,
                const pp_resolution_options_t *options) const {
    std::vector<pp_uuid_t> native_ids;
    native_ids.reserve(asset_ids.size());
    for (const Uuid &asset_id : asset_ids) {
      native_ids.push_back(detail::native_uuid(asset_id));
    }
    pp_resolution_set_t *raw_resolutions = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_production_resolve_assets(
        production_, native_ids.empty() ? nullptr : native_ids.data(),
        static_cast<std::uint64_t>(native_ids.size()), options,
        &raw_resolutions, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    detail::ResolutionSetHandle resolutions(raw_resolutions);

    return detail::resolution_values(std::move(resolutions));
  }

public:
  [[nodiscard]] Result<std::vector<Activity>> activities() const {
    pp_activity_set_t *raw_activities = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status =
        pp_production_activities(production_, &raw_activities, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    detail::ActivitySetHandle activities(raw_activities);

    std::vector<Activity> result;
    const std::uint64_t count = pp_activity_set_count(activities.get());
    result.reserve(static_cast<std::size_t>(count));
    for (std::uint64_t index = 0; index < count; ++index) {
      POSTPROJECT_TRY_ASSIGN(auto item_7,
                             detail::activity(activities.get(), index));
      result.push_back(std::move(item_7));
    }
    return result;
  }

  // Reads one job; an absent job is ErrorCode::not_found.
  [[nodiscard]] Result<Job> job(const Uuid &job_id) const {
    const pp_uuid_t native_id = detail::native_uuid(job_id);
    pp_job_set_t *raw_jobs = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status =
        pp_production_job(production_, &native_id, &raw_jobs, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    detail::JobSetHandle jobs(raw_jobs);
    if (pp_job_set_count(jobs.get()) != 1) {
      return Error(ErrorCode::internal, "job read returned no single job");
    }
    return detail::job(jobs.get(), 0);
  }

  [[nodiscard]] Result<QueryPage<Job>>
  jobs(std::uint32_t limit,
       std::optional<std::string_view> cursor = std::nullopt,
       std::optional<JobState> state = std::nullopt,
       std::optional<std::string_view> kind = std::nullopt) const {
    POSTPROJECT_TRY_ASSIGN(const std::optional<std::string> checked_cursor,
                           detail::checked_cursor(cursor));
    std::optional<std::string> checked_kind;
    if (kind.has_value()) {
      POSTPROJECT_TRY_ASSIGN(checked_kind,
                             detail::checked_string(*kind, "job kind"));
    }
    pp_job_set_t *raw_jobs = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_production_jobs(
        production_,
        state.has_value() ? static_cast<pp_job_state_t>(*state) : 0,
        checked_kind.has_value() ? checked_kind->c_str() : nullptr, limit,
        detail::optional_c_str(checked_cursor),
        &raw_jobs, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    detail::JobSetHandle jobs(raw_jobs);

    return detail::job_page(std::move(jobs));
  }

  [[nodiscard]] Result<std::vector<RegenerationJobPlan>>
  planRegeneration(const std::vector<Uuid> &artifact_representation_ids) const {
    std::vector<pp_uuid_t> native_ids;
    native_ids.reserve(artifact_representation_ids.size());
    for (const Uuid &id : artifact_representation_ids) {
      native_ids.push_back(detail::native_uuid(id));
    }
    pp_regeneration_plan_set_t *raw_plans = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_production_plan_regeneration(
        production_, native_ids.empty() ? nullptr : native_ids.data(),
        static_cast<std::uint64_t>(native_ids.size()), &raw_plans, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    detail::RegenerationPlanSetHandle plans(raw_plans);

    std::vector<RegenerationJobPlan> result;
    const std::uint64_t count = pp_regeneration_plan_set_count(plans.get());
    result.reserve(static_cast<std::size_t>(count));
    for (std::uint64_t index = 0; index < count; ++index) {
      pp_uuid_t artifact_id{};
      pp_job_set_t *raw_job = nullptr;
      pp_metadata_set_t *raw_parameters = nullptr;
      error = nullptr;
      const pp_error_code_t item_status = pp_regeneration_plan_set_get(
          plans.get(), index, &artifact_id, &raw_job, &raw_parameters, &error);
      POSTPROJECT_TRY(detail::check(item_status, error));
      detail::JobSetHandle job_set(raw_job);
      detail::MetadataSetHandle parameters(raw_parameters);
      if (pp_job_set_count(job_set.get()) != 1) {
        return Error(ErrorCode::internal,
                     "regeneration plan must contain one job");
      }
      POSTPROJECT_TRY_ASSIGN(Job job, detail::job(job_set.get(), 0));
      std::vector<RegenerationParameter> parameter_values;
      const std::uint64_t parameter_count =
          pp_metadata_set_count(parameters.get());
      parameter_values.reserve(static_cast<std::size_t>(parameter_count));
      for (std::uint64_t parameter_index = 0;
           parameter_index < parameter_count; ++parameter_index) {
        pp_object_ref_t target{};
        const char *vocabulary = nullptr;
        const char *property = nullptr;
        const pp_metadata_value_t *value = nullptr;
        error = nullptr;
        const pp_error_code_t parameter_status = pp_metadata_set_get(
            parameters.get(), parameter_index, &target, &vocabulary, &property,
            &value, &error);
        POSTPROJECT_TRY(detail::check(parameter_status, error));
        if (target.kind != PP_OBJECT_JOB || detail::uuid(target.id) != job.id) {
          return Error(ErrorCode::internal,
                       "regeneration parameter target does not match its job");
        }
        POSTPROJECT_TRY_ASSIGN(MetadataValue parameter,
                               detail::metadata_value(value));
        parameter_values.push_back({std::string(vocabulary),
                                    std::string(property),
                                    std::move(parameter)});
      }
      result.push_back({detail::uuid(artifact_id), std::move(job),
                        std::move(parameter_values)});
    }
    return result;
  }

  [[nodiscard]] Result<std::vector<Activity>>
  activitiesProducing(const Uuid &representation_id) const {
    return activities_for_representation(
        representation_id, pp_production_activities_producing);
  }

  [[nodiscard]] Result<std::vector<Activity>>
  activitiesConsuming(const Uuid &representation_id) const {
    return activities_for_representation(
        representation_id, pp_production_activities_consuming);
  }

  [[nodiscard]] Result<QueryPage<Activity>> activitiesProducing(
      const Uuid &representation_id, std::uint32_t limit,
      std::optional<std::string_view> cursor = std::nullopt) const {
    return activity_page_for_representation(
        representation_id, limit, cursor,
        pp_production_activities_producing_page);
  }

  [[nodiscard]] Result<QueryPage<Activity>> activitiesConsuming(
      const Uuid &representation_id, std::uint32_t limit,
      std::optional<std::string_view> cursor = std::nullopt) const {
    return activity_page_for_representation(
        representation_id, limit, cursor,
        pp_production_activities_consuming_page);
  }

  // Representations produced by activities of an exact kind.
  [[nodiscard]] Result<QueryPage<Uuid>> outputsByActivityKind(
      std::string_view kind, std::uint32_t limit,
      std::optional<std::string_view> cursor = std::nullopt) const {
    POSTPROJECT_TRY_ASSIGN(const std::string native_kind,
                           detail::checked_string(kind, "activity kind"));
    POSTPROJECT_TRY_ASSIGN(const std::optional<std::string> checked_cursor,
                           detail::checked_cursor(cursor));
    pp_object_query_set_t *raw_objects = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_production_outputs_by_activity_kind(
        production_, native_kind.c_str(), limit,
        detail::optional_c_str(checked_cursor), &raw_objects, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    POSTPROJECT_TRY_ASSIGN(
        QueryPage<ObjectMatch> page,
        detail::object_query_page(detail::ObjectQuerySetHandle(raw_objects)));
    return detail::object_id_page(std::move(page), ObjectKind::representation);
  }

  // Representations produced by activities with exactly this tool identity.
  [[nodiscard]] Result<QueryPage<Uuid>>
  outputsByTool(const ToolIdentity &tool, std::uint32_t limit,
                std::optional<std::string_view> cursor = std::nullopt) const {
    POSTPROJECT_TRY_ASSIGN(const std::string name,
                           detail::checked_string(tool.name, "tool name"));
    POSTPROJECT_TRY_ASSIGN(
        const std::optional<std::string> version,
        detail::checked_optional_string(tool.version, "tool version"));
    POSTPROJECT_TRY_ASSIGN(
        const std::optional<std::string> uri,
        detail::checked_optional_string(tool.uri, "tool URI"));
    POSTPROJECT_TRY_ASSIGN(const std::optional<std::string> checked_cursor,
                           detail::checked_cursor(cursor));
    pp_object_query_set_t *raw_objects = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_production_outputs_by_tool(
        production_, name.c_str(), detail::optional_c_str(version),
        detail::optional_c_str(uri), limit,
        detail::optional_c_str(checked_cursor), &raw_objects, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    POSTPROJECT_TRY_ASSIGN(
        QueryPage<ObjectMatch> page,
        detail::object_query_page(detail::ObjectQuerySetHandle(raw_objects)));
    return detail::object_id_page(std::move(page), ObjectKind::representation);
  }

  [[nodiscard]] Result<std::vector<Uuid>>
  ancestors(const Uuid &representation_id) const {
    return provenance_relatives(representation_id,
                                pp_production_provenance_ancestors);
  }

  [[nodiscard]] Result<std::vector<Uuid>>
  descendants(const Uuid &representation_id) const {
    return provenance_relatives(representation_id,
                                pp_production_provenance_descendants);
  }

  // Bounded provenance ancestors with their shortest depth.
  [[nodiscard]] Result<QueryPage<ObjectMatch>>
  ancestors(const Uuid &representation_id, std::uint32_t max_depth,
            std::uint32_t max_representations, std::uint32_t limit,
            std::optional<std::string_view> cursor = std::nullopt) const {
    return provenance_page(representation_id, max_depth, max_representations,
                           limit, cursor,
                           pp_production_provenance_ancestors_page);
  }

  // Bounded provenance descendants with their shortest depth.
  [[nodiscard]] Result<QueryPage<ObjectMatch>>
  descendants(const Uuid &representation_id, std::uint32_t max_depth,
              std::uint32_t max_representations, std::uint32_t limit,
              std::optional<std::string_view> cursor = std::nullopt) const {
    return provenance_page(representation_id, max_depth, max_representations,
                           limit, cursor,
                           pp_production_provenance_descendants_page);
  }

  [[nodiscard]] Result<ArtifactEvaluation>
  evaluateArtifact(const Uuid &representation_id, std::uint32_t max_depth = 64,
                   std::uint32_t max_representations = 1000) const {
    const pp_uuid_t native_id = detail::native_uuid(representation_id);
    pp_artifact_evaluation_t *raw_evaluation = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_production_evaluate_artifact(
        production_, &native_id, max_depth, max_representations,
        &raw_evaluation, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    detail::ArtifactEvaluationHandle evaluation(raw_evaluation);

    return detail::artifact_evaluation(std::move(evaluation));
  }

  [[nodiscard]] Result<ArtifactReproducibility>
  artifactReproducibility(const Uuid &representation_id) const {
    const pp_uuid_t native_id = detail::native_uuid(representation_id);
    pp_artifact_reproducibility_t *raw_report = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_production_artifact_reproducibility(
        production_, &native_id, &raw_report, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    detail::ArtifactReproducibilityHandle report(raw_report);

    return detail::artifact_reproducibility(std::move(report));
  }

  // Produced representations currently evaluated as stale. A source limits the
  // candidates to its provenance descendants.
  [[nodiscard]] Result<QueryPage<Uuid>> staleArtifacts(
      std::uint32_t evaluation_max_depth,
      std::uint32_t evaluation_max_representations, std::uint32_t limit,
      std::optional<std::string_view> cursor = std::nullopt,
      std::optional<Uuid> source_representation_id = std::nullopt) const {
    const std::optional<pp_uuid_t> native_source =
        source_representation_id.has_value()
            ? std::optional<pp_uuid_t>(
                  detail::native_uuid(*source_representation_id))
            : std::nullopt;
    POSTPROJECT_TRY_ASSIGN(const std::optional<std::string> checked_cursor,
                           detail::checked_cursor(cursor));
    pp_object_query_set_t *raw_objects = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_production_stale_artifacts(
        production_, native_source.has_value() ? &*native_source : nullptr,
        evaluation_max_depth, evaluation_max_representations, limit,
        detail::optional_c_str(checked_cursor), &raw_objects, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    POSTPROJECT_TRY_ASSIGN(
        QueryPage<ObjectMatch> page,
        detail::object_query_page(detail::ObjectQuerySetHandle(raw_objects)));
    return detail::object_id_page(std::move(page), ObjectKind::representation);
  }

  [[nodiscard]] Result<std::optional<Revision>> latestRevision() const {
    pp_revision_set_t *raw_revisions = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status =
        pp_production_latest_revision(production_, &raw_revisions, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    detail::RevisionSetHandle revisions(raw_revisions);
    if (pp_revision_set_count(revisions.get()) == 0) {
      return std::optional<Revision>();
    }
    POSTPROJECT_TRY_ASSIGN(Revision revision,
                           detail::revision(revisions.get(), 0));
    return std::optional<Revision>(std::move(revision));
  }

  [[nodiscard]] Result<std::vector<Revision>>
  changesSince(std::uint64_t sequence, std::uint32_t limit = 100) const {
    pp_revision_set_t *raw_revisions = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_production_changes_since(
        production_, sequence, limit, &raw_revisions, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    detail::RevisionSetHandle revisions(raw_revisions);
    return detail::revisions(revisions.get());
  }

  // Revisions after `sequence` with at least one event of `kinds`; continue
  // from the returned through sequence.
  [[nodiscard]] Result<FilteredRevisionPage>
  changesSinceFiltered(std::uint64_t sequence,
                       const std::vector<RevisionEventKind> &kinds,
                       std::uint32_t limit = 100) const {
    std::vector<pp_revision_event_kind_t> native_kinds;
    native_kinds.reserve(kinds.size());
    for (const RevisionEventKind kind : kinds) {
      native_kinds.push_back(static_cast<pp_revision_event_kind_t>(kind));
    }
    pp_revision_set_t *raw_revisions = nullptr;
    std::uint64_t through_sequence = 0;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_production_changes_since_filtered(
        production_, sequence, native_kinds.data(),
        static_cast<std::uint64_t>(native_kinds.size()), limit,
        &raw_revisions, &through_sequence, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    detail::RevisionSetHandle revisions(raw_revisions);
    POSTPROJECT_TRY_ASSIGN(std::vector<Revision> items,
                           detail::revisions(revisions.get()));
    return FilteredRevisionPage{std::move(items), through_sequence};
  }

  [[nodiscard]] Result<RevisionWaiter> revisionWaiter() const {
    pp_revision_waiter_t *waiter = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status =
        pp_revision_waiter_create(production_, &waiter, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return RevisionWaiter(waiter);
  }

  // Distinct semantic objects touched by revisions after `sequence`.
  [[nodiscard]] Result<QueryPage<ObjectRef>> objectsChangedSince(
      std::uint64_t sequence, std::uint32_t limit,
      std::optional<std::string_view> cursor = std::nullopt) const {
    POSTPROJECT_TRY_ASSIGN(const std::optional<std::string> checked_cursor,
                           detail::checked_cursor(cursor));
    pp_object_query_set_t *raw_objects = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_production_objects_changed_since(
        production_, sequence, limit, detail::optional_c_str(checked_cursor),
        &raw_objects, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    POSTPROJECT_TRY_ASSIGN(
        QueryPage<ObjectMatch> page,
        detail::object_query_page(detail::ObjectQuerySetHandle(raw_objects)));
    return detail::object_ref_page(std::move(page));
  }

  [[nodiscard]] Result<std::vector<RevisionEvent>>
  revisionEvents(const Uuid &revision_id) const {
    const pp_uuid_t native_id = detail::native_uuid(revision_id);
    pp_revision_event_set_t *raw_events = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_production_revision_events(
        production_, &native_id, &raw_events, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    detail::RevisionEventSetHandle events(raw_events);
    std::vector<RevisionEvent> result;
    const std::uint64_t count = pp_revision_event_set_count(events.get());
    result.reserve(static_cast<std::size_t>(count));
    for (std::uint64_t index = 0; index < count; ++index) {
      POSTPROJECT_TRY_ASSIGN(auto item_12,
                             detail::revision_event(events.get(), index));
      result.push_back(std::move(item_12));
    }
    return result;
  }

  [[nodiscard]] Result<Transaction> beginTransaction() {
    pp_transaction_t *transaction = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status =
        pp_production_begin_transaction(production_, &transaction, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return Transaction(transaction);
  }

  [[nodiscard]] Result<Transaction> beginTransaction(const Uuid &base_revision) {
    const pp_uuid_t native_base = detail::native_uuid(base_revision);
    pp_transaction_t *transaction = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_production_begin_transaction_at(
        production_, &native_base, &transaction, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return Transaction(transaction);
  }

  [[nodiscard]] explicit operator bool() const noexcept {
    return production_ != nullptr;
  }

private:
  using ActivityQuery = pp_error_code_t (*)(
      const pp_production_t *, const pp_uuid_t *, pp_activity_set_t **,
      pp_error_t **);
  using ProvenanceQuery = pp_error_code_t (*)(
      const pp_production_t *, const pp_uuid_t *, pp_object_ref_set_t **,
      pp_error_t **);
  using ActivityPageQuery = pp_error_code_t (*)(
      const pp_production_t *, const pp_uuid_t *, std::uint32_t, const char *,
      pp_activity_set_t **, pp_error_t **);
  using ProvenancePageQuery = pp_error_code_t (*)(
      const pp_production_t *, const pp_uuid_t *, std::uint32_t,
      std::uint32_t, std::uint32_t, const char *, pp_object_query_set_t **,
      pp_error_t **);

  explicit Production(pp_production_t *production) noexcept : production_(production) {}

  [[nodiscard]] Result<std::vector<Activity>>
  activities_for_representation(const Uuid &representation_id,
                                ActivityQuery query) const {
    const pp_uuid_t native_id = detail::native_uuid(representation_id);
    pp_activity_set_t *raw_activities = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status =
        query(production_, &native_id, &raw_activities, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    detail::ActivitySetHandle activities(raw_activities);

    std::vector<Activity> result;
    const std::uint64_t count = pp_activity_set_count(activities.get());
    result.reserve(static_cast<std::size_t>(count));
    for (std::uint64_t index = 0; index < count; ++index) {
      POSTPROJECT_TRY_ASSIGN(auto item_8,
                             detail::activity(activities.get(), index));
      result.push_back(std::move(item_8));
    }
    return result;
  }

  [[nodiscard]] Result<QueryPage<Activity>> activity_page_for_representation(
      const Uuid &representation_id, std::uint32_t limit,
      const std::optional<std::string_view> &cursor,
      ActivityPageQuery query) const {
    const pp_uuid_t native_id = detail::native_uuid(representation_id);
    POSTPROJECT_TRY_ASSIGN(const std::optional<std::string> checked_cursor,
                           detail::checked_cursor(cursor));
    pp_activity_set_t *raw_activities = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status =
        query(production_, &native_id, limit,
              detail::optional_c_str(checked_cursor), &raw_activities, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return detail::activity_page(detail::ActivitySetHandle(raw_activities));
  }

  [[nodiscard]] Result<QueryPage<ObjectMatch>>
  provenance_page(const Uuid &representation_id, std::uint32_t max_depth,
                  std::uint32_t max_representations, std::uint32_t limit,
                  const std::optional<std::string_view> &cursor,
                  ProvenancePageQuery query) const {
    const pp_uuid_t native_id = detail::native_uuid(representation_id);
    POSTPROJECT_TRY_ASSIGN(const std::optional<std::string> checked_cursor,
                           detail::checked_cursor(cursor));
    pp_object_query_set_t *raw_objects = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status =
        query(production_, &native_id, max_depth, max_representations, limit,
              detail::optional_c_str(checked_cursor), &raw_objects, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return detail::object_query_page(detail::ObjectQuerySetHandle(raw_objects));
  }

  [[nodiscard]] Result<QueryPage<MetadataAssertion>>
  query_metadata_impl(std::string_view vocabulary, std::string_view property,
                      const pp_metadata_input_t *exact_value,
                      std::uint32_t limit,
                      const std::optional<std::string_view> &cursor) const {
    POSTPROJECT_TRY_ASSIGN(
        const std::string native_vocabulary,
        detail::checked_string(vocabulary, "metadata vocabulary"));
    POSTPROJECT_TRY_ASSIGN(
        const std::string native_property,
        detail::checked_string(property, "metadata property"));
    POSTPROJECT_TRY_ASSIGN(const std::optional<std::string> checked_cursor,
                           detail::checked_cursor(cursor));
    pp_metadata_set_t *raw_metadata = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status = pp_production_query_metadata(
        production_, native_vocabulary.c_str(), native_property.c_str(),
        exact_value, limit, detail::optional_c_str(checked_cursor),
        &raw_metadata, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return detail::metadata_page(detail::MetadataSetHandle(raw_metadata));
  }

  [[nodiscard]] Result<std::vector<Uuid>>
  provenance_relatives(const Uuid &representation_id,
                       ProvenanceQuery query) const {
    const pp_uuid_t native_id = detail::native_uuid(representation_id);
    pp_object_ref_set_t *raw_objects = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status =
        query(production_, &native_id, &raw_objects, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    detail::ObjectRefSetHandle objects(raw_objects);

    std::vector<Uuid> result;
    const std::uint64_t count = pp_object_ref_set_count(objects.get());
    result.reserve(static_cast<std::size_t>(count));
    for (std::uint64_t index = 0; index < count; ++index) {
      pp_object_ref_t object{};
      pp_error_t *item_error = nullptr;
      const pp_error_code_t item_status =
          pp_object_ref_set_get(objects.get(), index, &object, &item_error);
      POSTPROJECT_TRY(detail::check(item_status, item_error));
      if (object.kind != PP_OBJECT_REPRESENTATION) {
        return Error(ErrorCode::internal,
                     "provenance query returned a non-representation object");
      }
      result.push_back(detail::uuid(object.id));
    }
    return result;
  }

  static Result<Production> create_impl(std::string_view path,
                                        const char *display_name) {
    POSTPROJECT_TRY_ASSIGN(const std::string native_path,
                           detail::checked_string(path, "path"));
    pp_production_t *production = nullptr;
    pp_error_t *error = nullptr;
    const pp_error_code_t status =
        pp_production_create(native_path.c_str(), display_name, &production, &error);
    POSTPROJECT_TRY(detail::check(status, error));
    return Production(production);
  }

  pp_production_t *production_ = nullptr;
};

// Calls `callback` for each new revision on a thread the observer owns. With
// `kinds`, only revisions containing one of those event kinds are delivered.
// Stop the observer (or destroy it) before destroying the production; do not
// destroy it from its own callback. A failed query, or with exceptions enabled
// an exception from the callback, ends observation and is available from
// error().
class RevisionObserver final {
public:
  using Callback = std::function<void(const Revision &,
                                      const std::vector<RevisionEvent> &)>;

  // Starts delivering on a thread the observer owns. If the observer cannot
  // start, error() reports why and no thread runs.
  RevisionObserver(const Production &production, std::uint64_t after_sequence,
                   Callback callback, std::vector<RevisionEventKind> kinds = {})
      : production_(production), cursor_(after_sequence),
        callback_(std::move(callback)), kinds_(std::move(kinds)) {
    Result<RevisionWaiter> waiter = production.revisionWaiter();
    if (!waiter.has_value()) {
      error_ = waiter.error();
      return;
    }
    waiter_.emplace(*std::move(waiter));
    thread_ = std::thread([this] { run(); });
  }

  RevisionObserver(const RevisionObserver &) = delete;
  RevisionObserver &operator=(const RevisionObserver &) = delete;
  RevisionObserver(RevisionObserver &&) = delete;
  RevisionObserver &operator=(RevisionObserver &&) = delete;

  ~RevisionObserver() { stop(); }

  // Cancels the wait and joins the observer thread. Safe to call repeatedly;
  // from the callback it only cancels.
  void stop() noexcept {
    if (waiter_.has_value()) {
      waiter_->cancel();
    }
    if (thread_.joinable() && thread_.get_id() != std::this_thread::get_id()) {
      thread_.join();
    }
  }

  // Sequence of the last fully delivered revision or filtered page.
  [[nodiscard]] std::uint64_t cursor() const noexcept { return cursor_.load(); }

  // The error that stopped delivery, if any. With exceptions enabled, an
  // exception escaping the callback is reported as ErrorCode::internal.
  [[nodiscard]] std::optional<Error> error() const {
    const std::lock_guard<std::mutex> lock(mutex_);
    return error_;
  }

private:
  static constexpr std::uint32_t page_size = 100;

  void run() noexcept {
#if POSTPROJECT_HAS_EXCEPTIONS
    try {
      record(deliver());
    } catch (const std::exception &exception) {
      record(Error(ErrorCode::internal, exception.what()));
    } catch (...) {
      record(Error(ErrorCode::internal, "revision callback failed"));
    }
#else
    record(deliver());
#endif
  }

  void record(Result<void> result) {
    if (!result.has_value()) {
      const std::lock_guard<std::mutex> lock(mutex_);
      error_ = result.error();
    }
  }

  Result<void> deliver() {
    while (true) {
      POSTPROJECT_TRY_ASSIGN(const RevisionWait wait,
                             waiter_->wait(cursor_.load(), page_size));
      if (wait.result == RevisionWaitResult::timed_out) {
        continue;
      }
      if (wait.result != RevisionWaitResult::revisions) {
        return {};
      }
      if (kinds_.empty()) {
        for (const Revision &revision : wait.revisions) {
          POSTPROJECT_TRY_ASSIGN(const std::vector<RevisionEvent> events,
                                 production_.revisionEvents(revision.id));
          callback_(revision, events);
          cursor_.store(revision.sequence);
        }
        continue;
      }
      while (true) {
        POSTPROJECT_TRY_ASSIGN(const FilteredRevisionPage page,
                               production_.changesSinceFiltered(
                                   cursor_.load(), kinds_, page_size));
        for (const Revision &revision : page.revisions) {
          POSTPROJECT_TRY_ASSIGN(const std::vector<RevisionEvent> events,
                                 production_.revisionEvents(revision.id));
          callback_(revision, events);
        }
        cursor_.store(page.through_sequence);
        if (page.revisions.size() < page_size) {
          break;
        }
      }
    }
  }

  const Production &production_;
  std::optional<RevisionWaiter> waiter_;
  std::atomic<std::uint64_t> cursor_;
  Callback callback_;
  std::vector<RevisionEventKind> kinds_;
  mutable std::mutex mutex_;
  std::optional<Error> error_;
  std::thread thread_;
};

[[nodiscard]] inline std::uint32_t abi_version() noexcept {
  return pp_abi_version();
}

namespace detail {

inline Result<std::string> owned_string(pp_error_code_t status, char *raw,
                                        pp_error_t *error) {
  StringHandle value(raw);
  POSTPROJECT_TRY(check(status, error));
  return std::string(value.get());
}

} // namespace detail

// Returns the canonical file: locator URI import records for an existing
// path. Compare locators only through URIs from PostProject.
[[nodiscard]] inline Result<std::string> fileLocator(std::string_view path) {
  POSTPROJECT_TRY_ASSIGN(const std::string native_path,
                         detail::checked_string(path, "path"));
  char *uri = nullptr;
  pp_error_t *error = nullptr;
  const pp_error_code_t status =
      pp_file_path_to_locator(native_path.c_str(), &uri, &error);
  return detail::owned_string(status, uri, error);
}

// Converts a local file: locator URI to a native path, which need not exist.
[[nodiscard]] inline Result<std::string> locatorFilePath(std::string_view uri) {
  POSTPROJECT_TRY_ASSIGN(const std::string native_uri,
                         detail::checked_string(uri, "uri"));
  char *path = nullptr;
  pp_error_t *error = nullptr;
  const pp_error_code_t status =
      pp_locator_to_file_path(native_uri.c_str(), &path, &error);
  return detail::owned_string(status, path, error);
}

// Computes the fingerprint import records for a regular file.
[[nodiscard]] inline Result<Fingerprint>
fingerprintFile(std::string_view path) {
  POSTPROJECT_TRY_ASSIGN(const std::string native_path,
                         detail::checked_string(path, "path"));
  pp_fingerprint_t *raw = nullptr;
  pp_error_t *error = nullptr;
  POSTPROJECT_TRY(detail::check(
      pp_fingerprint_file(native_path.c_str(), &raw, &error), error));
  const std::unique_ptr<pp_fingerprint_t, void (*)(pp_fingerprint_t *)>
      fingerprint(raw, pp_fingerprint_release);
  const char *algorithm = nullptr;
  std::uint16_t version = 0;
  const std::uint8_t *value = nullptr;
  std::uint64_t length = 0;
  POSTPROJECT_TRY(
      detail::check(pp_fingerprint_get(fingerprint.get(), &algorithm, &version,
                                       &value, &length, &error),
                    error));
  return Fingerprint{algorithm, version,
                     std::vector<std::uint8_t>(
                         value, value + static_cast<std::size_t>(length))};
}

} // namespace postproject

template <> struct std::hash<postproject::ProductionId> {
  std::size_t operator()(const postproject::ProductionId &id) const noexcept {
    const auto &bytes = id.bytes();
    return std::hash<std::string_view>{}(std::string_view(
        reinterpret_cast<const char *>(bytes.data()), bytes.size()));
  }
};

#endif
