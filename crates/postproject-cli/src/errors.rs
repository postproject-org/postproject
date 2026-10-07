//! Stable process exit categories; diagnostics remain descriptive text.

use postproject_core::ErrorKind;

pub(crate) fn invalid(message: &'static str) -> postproject_core::Error {
    postproject_core::Error::new(ErrorKind::InvalidArgument, message)
}

pub(crate) fn category(error: &anyhow::Error) -> (&'static str, u8) {
    if let Some(error) = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<postproject_core::Error>())
    {
        return match error.kind() {
            ErrorKind::InvalidArgument => ("invalid_argument", 2),
            ErrorKind::NotFound => ("not_found", 3),
            ErrorKind::Conflict => ("conflict", 4),
            ErrorKind::AlreadyExists => ("already_exists", 4),
            ErrorKind::AmbiguousResolution => ("ambiguous_resolution", 5),
            ErrorKind::Cancelled => ("cancelled", 6),
            ErrorKind::Io => ("io", 7),
            ErrorKind::Storage => ("storage", 7),
            ErrorKind::Migration => ("migration", 7),
            ErrorKind::Fingerprint => ("fingerprint", 7),
            ErrorKind::Unsupported => ("unsupported", 8),
            ErrorKind::Internal => ("internal", 9),
            _ => ("operation_failed", 1),
        };
    }
    if error
        .chain()
        .any(<dyn std::error::Error>::is::<std::io::Error>)
    {
        return ("io", 7);
    }
    ("operation_failed", 1)
}
