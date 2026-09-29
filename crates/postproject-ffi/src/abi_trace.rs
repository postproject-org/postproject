//! Opt-in, process-local evidence that exported C ABI operations were reached.

use std::{
    collections::BTreeSet,
    env, fs,
    io::Write,
    panic::{AssertUnwindSafe, catch_unwind},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

const TRACE_ENVIRONMENT_VARIABLE: &str = "POSTPROJECT_ABI_TRACE";

struct Trace {
    path: PathBuf,
    operations: Mutex<BTreeSet<&'static str>>,
}

static TRACE: OnceLock<Option<Trace>> = OnceLock::new();

/// Records one exported operation when ABI tracing was enabled before first use.
pub(crate) fn record(operation: &'static str) {
    let _ = catch_unwind(AssertUnwindSafe(|| record_inner(operation)));
}

fn record_inner(operation: &'static str) {
    let Some(trace) = TRACE.get_or_init(configured_trace) else {
        return;
    };
    let Ok(mut operations) = trace.operations.lock() else {
        return;
    };
    if operations.insert(operation) {
        let _ = write_trace(&trace.path, &operations);
    }
}

fn configured_trace() -> Option<Trace> {
    let path = env::var_os(TRACE_ENVIRONMENT_VARIABLE)?;
    if path.is_empty() {
        return None;
    }
    Some(Trace {
        path: PathBuf::from(path),
        operations: Mutex::new(BTreeSet::new()),
    })
}

fn write_trace(path: &Path, operations: &BTreeSet<&str>) -> std::io::Result<()> {
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    let mut file = fs::File::create(&temporary)?;
    for operation in operations {
        writeln!(file, "{operation}")?;
    }
    file.flush()?;
    fs::rename(temporary, path)
}

#[cfg(test)]
mod tests {
    use super::write_trace;
    use std::collections::BTreeSet;

    #[test]
    fn trace_output_is_sorted_and_deduplicated() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("trace.txt");
        let operations = BTreeSet::from([
            "pp_transaction_commit",
            "pp_production_open",
            "pp_transaction_commit",
        ]);

        write_trace(&path, &operations).expect("write trace");

        assert_eq!(
            std::fs::read_to_string(path).expect("read trace"),
            "pp_production_open\npp_transaction_commit\n"
        );
    }
}
