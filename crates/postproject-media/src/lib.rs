//! Filesystem-facing media operations for `PostProject`.
//!
//! Includes deterministic fingerprints, discovery and resolution policy,
//! optional technical inspection, and bounded local execution adapters.

#![forbid(unsafe_code)]

mod content;
mod executor;
mod fingerprint;
mod import;
mod inspection;
mod inventory;
mod path;
mod recognition;
mod representation_fingerprint;
mod resolver;
mod sequence_fingerprint;

pub use content::{
    ContentObservation, ContentObservationOutcome, ContentVerification,
    fingerprint_resource_content, observe_resource_content, resource_usage,
    verify_resource_content,
};
pub use executor::{
    EXECUTOR_PARAMETER_VOCABULARY, EXECUTOR_PROFILE_PROPERTY, ExecutionOutcome, ExecutionRequest,
    Executor, ExecutorCapability, FfmpegExecutor, GENERATE_PROXY_JOB_KIND,
    GENERATE_THUMBNAIL_JOB_KIND, PROXY_720P_PROFILE, PROXY_1080P_PROFILE, THUMBNAIL_640_PROFILE,
    THUMBNAIL_1280_PROFILE,
};
pub use fingerprint::{
    FULL_FINGERPRINT_ALGORITHM, FULL_HASH_LIMIT_BYTES, FingerprintCoverage, FingerprintReport,
    REGION_SIZE_BYTES, SAMPLED_FINGERPRINT_ALGORITHM, fingerprint_file,
};
pub use import::{
    FileResourceSource, ImageSequenceSource, prepare_confirmed_locator,
    prepare_confirmed_locator_under_root, prepare_image_sequence_representation,
    prepare_media_root, prepare_ordered_parts_representation, prepare_original_media,
    prepare_package_representation, prepare_single_file_representation,
};
pub use inspection::{
    FfprobeInspector, InspectionOutcome, MediaInspector, TECHNICAL_INSPECTION_PROPERTY,
    TECHNICAL_METADATA_VOCABULARY, TechnicalMetadata,
};
pub use inventory::{
    InventoryCategory, InventoryItem, InventoryReport, InventoryScanner, InventoryStats,
};
pub use path::{canonical_file_uri, local_file_path};
pub use recognition::{
    AVCHD_CLIP_INFO_ROLE, AVCHD_ESSENCE_ROLE, AVCHD_NAVIGATION_ROLE, AVCHD_PLAYLIST_ROLE,
    MediaRecognizer, PRIMARY_ESSENCE_ROLE, RecognizedMedia, RecognizedMember, SIDECAR_ROLE,
    SPAN_PART_ROLE, prepare_recognized_original_media,
};
pub use representation_fingerprint::{
    REPRESENTATION_FINGERPRINT_ALGORITHM, REPRESENTATION_FINGERPRINT_VERSION,
    fingerprint_representation,
};
pub use resolver::{
    MediaResolver, MediaRootMapping, ResolutionItem, ResolverOptions, SearchScope, VerificationMode,
};
pub use sequence_fingerprint::{
    SEQUENCE_FINGERPRINT_ALGORITHM, SEQUENCE_FINGERPRINT_VERSION, SequenceFingerprintReport,
    fingerprint_image_sequence,
};
