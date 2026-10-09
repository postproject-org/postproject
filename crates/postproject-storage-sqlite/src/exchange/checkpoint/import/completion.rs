//! Private ownership and completion evidence; never part of portable state.

use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use postproject_protocol::{CheckpointManifest, Document, Limits};

use crate::ExchangeResult;

use super::{super::invalid, limits::budget, staging::io_error};

const OWNER: &str = "PostProject checkpoint staging v1\n";
const MAX_OWNER_BYTES: u64 = 262_144;

pub(super) fn create(
    directory: &Path,
    file: &Path,
    manifest: &CheckpointManifest,
) -> ExchangeResult<()> {
    let filename = file
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(invalid)?;
    let mut owner = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join("owner"))
        .map_err(|error| io_error(&error))?;
    owner
        .write_all(OWNER.as_bytes())
        .and_then(|()| owner.write_all(filename.as_bytes()))
        .and_then(|()| owner.write_all(b"\n"))
        .map_err(|error| io_error(&error))?;
    owner
        .write_all(&manifest.document()?.canonical_bytes()?)
        .and_then(|()| owner.sync_all())
        .map_err(|error| io_error(&error))?;
    Ok(())
}

pub(super) fn seal(directory: &Path, file: &Path) -> ExchangeResult<()> {
    let digest = digest(directory, file)?;
    let mut complete = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join("complete.tmp"))
        .map_err(|error| io_error(&error))?;
    complete
        .write_all(digest.as_bytes())
        .and_then(|()| complete.sync_all())
        .map_err(|error| io_error(&error))?;
    drop(complete);
    std::fs::rename(directory.join("complete.tmp"), directory.join("complete"))
        .map_err(|error| io_error(&error))?;
    Ok(())
}

pub(super) struct OwnedStage {
    pub(super) file: PathBuf,
    pub(super) manifest: CheckpointManifest,
    pub(super) complete: bool,
    pub(super) entries: Vec<PathBuf>,
}

pub(super) fn inspect(directory: &Path, maximum: u64) -> ExchangeResult<OwnedStage> {
    if !directory
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with(".postproject-import-") && name.len() > 20)
        || !std::fs::symlink_metadata(directory)
            .map_err(|error| io_error(&error))?
            .file_type()
            .is_dir()
    {
        return Err(invalid().into());
    }
    let owner =
        std::fs::symlink_metadata(directory.join("owner")).map_err(|error| io_error(&error))?;
    if !owner.file_type().is_file() || owner.len() > MAX_OWNER_BYTES {
        return Err(invalid().into());
    }
    let mut bytes = Vec::new();
    File::open(directory.join("owner"))
        .map_err(|error| io_error(&error))?
        .take(MAX_OWNER_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| io_error(&error))?;
    let content = std::str::from_utf8(&bytes)
        .map_err(|_| invalid())?
        .strip_prefix(OWNER)
        .ok_or_else(invalid)?;
    let (filename, document) = content.split_once('\n').ok_or_else(invalid)?;
    if !filename.starts_with("mirror-")
        || Path::new(filename).extension() != Some(std::ffi::OsStr::new("pproj"))
        || !filename
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
    {
        return Err(invalid().into());
    }
    let manifest = CheckpointManifest::from_document(&Document::parse(
        document.as_bytes(),
        Limits::new(262_144, 192, 8192)?,
    )?)?;
    let mut entries = Vec::new();
    let mut total = 0_u64;
    for entry in std::fs::read_dir(directory).map_err(|error| io_error(&error))? {
        let entry = entry.map_err(|error| io_error(&error))?;
        let metadata = std::fs::symlink_metadata(entry.path()).map_err(|error| io_error(&error))?;
        let name = entry.file_name();
        let name = name.to_str().ok_or_else(invalid)?;
        if !metadata.file_type().is_file()
            || ![
                filename.to_owned(),
                format!("{filename}-journal"),
                format!("{filename}-wal"),
                format!("{filename}-shm"),
                "owner".into(),
                "complete".into(),
                "complete.tmp".into(),
            ]
            .iter()
            .any(|allowed| allowed == name)
        {
            return Err(invalid().into());
        }
        total = total.checked_add(metadata.len()).ok_or_else(budget)?;
        if total > maximum {
            return Err(budget().into());
        }
        entries.push(entry.path());
    }
    let file = directory.join(filename);
    let complete_path = directory.join("complete");
    let complete = complete_path
        .try_exists()
        .map_err(|error| io_error(&error))?;
    if complete {
        let size = std::fs::metadata(&complete_path)
            .map_err(|error| io_error(&error))?
            .len();
        if size != 64
            || std::fs::read(complete_path).map_err(|error| io_error(&error))?
                != digest(directory, &file)?.as_bytes()
        {
            return Err(invalid().into());
        }
        // A sealed stage has closed all SQLite/journal handles.
        if entries.iter().any(|entry| {
            entry.extension().is_some_and(|extension| {
                extension == "pproj-journal" || extension == "pproj-wal" || extension == "pproj-shm"
            })
        }) {
            return Err(invalid().into());
        }
    }
    Ok(OwnedStage {
        file,
        manifest,
        complete,
        entries,
    })
}

fn digest(directory: &Path, file: &Path) -> ExchangeResult<String> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"PostProject private checkpoint completion\0");
    let owner = directory.join("owner");
    let mut buffer = vec![0; 64 * 1024];
    for path in [&owner, file] {
        let mut input = File::open(path).map_err(|error| io_error(&error))?;
        hasher.update(
            &input
                .metadata()
                .map_err(|error| io_error(&error))?
                .len()
                .to_be_bytes(),
        );
        loop {
            let count = input.read(&mut buffer).map_err(|error| io_error(&error))?;
            if count == 0 {
                break;
            }
            hasher.update(&buffer[..count]);
        }
    }
    Ok(hasher.finalize().to_hex().to_string())
}
