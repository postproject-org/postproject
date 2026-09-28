//! Filesystem adapter for recognizing compound media layouts.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use postproject_core::{
    Asset, AssetId, Error, ErrorKind, FrameRange, OriginalMediaImport, RationalRate,
    RepresentationImport, RepresentationKind, ResourceRole, Result, SequenceNaming, Timestamp,
};

use crate::{FileResourceSource, ImageSequenceSource, MediaSource, prepare_representation};

/// Role assigned to the playable files in an AVCHD package.
pub const AVCHD_ESSENCE_ROLE: &str = "org.postproject.avchd:essence";
/// Role assigned to AVCHD clip-information sidecars.
pub const AVCHD_CLIP_INFO_ROLE: &str = "org.postproject.avchd:clip-info";
/// Role assigned to AVCHD playlists.
pub const AVCHD_PLAYLIST_ROLE: &str = "org.postproject.avchd:playlist";
/// Role assigned to other AVCHD navigation and index files.
pub const AVCHD_NAVIGATION_ROLE: &str = "org.postproject.avchd:navigation";
/// Role assigned to ordered recording spans.
pub const SPAN_PART_ROLE: &str = "org.postproject:span-part";
/// Role assigned to a primary file when sidecars form a package.
pub const PRIMARY_ESSENCE_ROLE: &str = "org.postproject:essence";
/// Role assigned to metadata sidecars beside a primary file.
pub const SIDECAR_ROLE: &str = "org.postproject:sidecar";

/// One recognized package member and its open-world role.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecognizedMember {
    path: PathBuf,
    role: String,
    required: bool,
}

impl RecognizedMember {
    fn new(path: PathBuf, role: &str, required: bool) -> Self {
        Self {
            path,
            role: role.to_owned(),
            required,
        }
    }

    /// Returns the member path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the namespaced member role.
    #[must_use]
    pub fn role(&self) -> &str {
        &self.role
    }

    /// Returns whether the representation requires this member.
    #[must_use]
    pub const fn is_required(&self) -> bool {
        self.required
    }
}

/// A filesystem layout recognized without mutating a production.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RecognizedMedia {
    /// One regular file with no recognized companions.
    SingleFile(PathBuf),
    /// A compact numbered image sequence.
    ImageSequence {
        /// Directory containing the frames.
        directory: PathBuf,
        /// Naming shared by every frame file in the directory.
        naming: SequenceNaming,
        /// Inclusive frame-number domain.
        frames: FrameRange,
        /// Frames absent within the domain.
        missing_frames: Vec<i64>,
        /// Caller-supplied interpretation rate.
        rate: RationalRate,
    },
    /// Files whose ordering is semantically significant.
    OrderedParts(Vec<RecognizedMember>),
    /// Role-bearing camera-card or sidecar package.
    Package(Vec<RecognizedMember>),
}

/// Adapter that recognizes supported on-disk media conventions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MediaRecognizer {
    sequence_rate: RationalRate,
}

impl MediaRecognizer {
    /// Creates a recognizer with the rate used for filename-only sequences.
    #[must_use]
    pub const fn new(sequence_rate: RationalRate) -> Self {
        Self { sequence_rate }
    }

    /// Returns every credible interpretation in deterministic order.
    ///
    /// A directory containing several unrelated numbered groups returns several
    /// candidates rather than silently choosing one.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the supplied path cannot be inspected or a
    /// domain error when a recognized pattern cannot be represented safely.
    pub fn recognize(&self, path: impl AsRef<Path>) -> Result<Vec<RecognizedMedia>> {
        let path = path.as_ref();
        let metadata = fs::metadata(path).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot inspect media path {}: {error}", path.display()),
            )
        })?;
        if metadata.is_file() {
            return recognize_file(path);
        }
        if !metadata.is_dir() {
            return Err(Error::new(
                ErrorKind::InvalidArgument,
                format!("media path is not a file or directory: {}", path.display()),
            ));
        }
        if let Some(package) = recognize_avchd(path)? {
            return Ok(vec![package]);
        }
        recognize_numbered_groups(path, self.sequence_rate)
    }
}

/// Builds an original-media import from one explicit recognition result.
///
/// # Errors
///
/// Returns filesystem, fingerprint, or domain validation errors while preparing
/// the canonical representation aggregate.
pub fn prepare_recognized_original_media(
    recognized: &RecognizedMedia,
    display_name: Option<String>,
    import_source: Option<String>,
) -> Result<OriginalMediaImport> {
    let asset = Asset::new(
        AssetId::new(),
        Timestamp::now()?,
        display_name,
        import_source,
    );
    let prepared =
        prepare_recognized_representation(asset.id(), RepresentationKind::Original, recognized)?;
    let (representation, resources, locators) = prepared.into_parts();
    OriginalMediaImport::new(asset, representation, resources, locators)
}

fn prepare_recognized_representation(
    asset_id: AssetId,
    kind: RepresentationKind,
    recognized: &RecognizedMedia,
) -> Result<RepresentationImport> {
    let source = match recognized {
        RecognizedMedia::SingleFile(path) => MediaSource::File(path.clone()),
        RecognizedMedia::ImageSequence {
            directory,
            naming,
            frames,
            missing_frames,
            rate,
        } => MediaSource::ImageSequence(ImageSequenceSource::new(
            directory,
            naming.clone(),
            *frames,
            *rate,
            missing_frames.clone(),
        )),
        RecognizedMedia::OrderedParts(members) => MediaSource::OrderedParts(file_sources(members)?),
        RecognizedMedia::Package(members) => MediaSource::Package(file_sources(members)?),
    };
    prepare_representation(asset_id, kind, source)
}

fn file_sources(members: &[RecognizedMember]) -> Result<Vec<FileResourceSource>> {
    members
        .iter()
        .map(|member| {
            Ok(FileResourceSource::new(
                &member.path,
                ResourceRole::new(&member.role)?,
                member.required,
            ))
        })
        .collect()
}

fn recognize_file(path: &Path) -> Result<Vec<RecognizedMedia>> {
    let stem = path.file_stem().and_then(|stem| stem.to_str());
    let Some(stem) = stem else {
        return Ok(vec![RecognizedMedia::SingleFile(path.to_path_buf())]);
    };
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut sidecars = sorted_files(parent)?
        .into_iter()
        .filter(|candidate| candidate != path)
        .filter(|candidate| candidate.file_stem().and_then(|value| value.to_str()) == Some(stem))
        .filter(|candidate| {
            candidate
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|extension| {
                    matches!(
                        extension.to_ascii_lowercase().as_str(),
                        "xmp" | "xml" | "json"
                    )
                })
        })
        .collect::<Vec<_>>();
    if sidecars.is_empty() {
        return Ok(vec![RecognizedMedia::SingleFile(path.to_path_buf())]);
    }
    let mut members = vec![RecognizedMember::new(
        path.to_path_buf(),
        PRIMARY_ESSENCE_ROLE,
        true,
    )];
    members.extend(
        sidecars
            .drain(..)
            .map(|path| RecognizedMember::new(path, SIDECAR_ROLE, false)),
    );
    Ok(vec![RecognizedMedia::Package(members)])
}

fn recognize_avchd(root: &Path) -> Result<Option<RecognizedMedia>> {
    let bdmv = root.join("PRIVATE/AVCHD/BDMV");
    if !bdmv.is_dir() {
        return Ok(None);
    }
    let mut members = Vec::new();
    append_role_files(&mut members, &bdmv.join("STREAM"), AVCHD_ESSENCE_ROLE, true)?;
    append_role_files(
        &mut members,
        &bdmv.join("CLIPINF"),
        AVCHD_CLIP_INFO_ROLE,
        false,
    )?;
    append_role_files(
        &mut members,
        &bdmv.join("PLAYLIST"),
        AVCHD_PLAYLIST_ROLE,
        false,
    )?;
    for name in ["INDEX.BDM", "MovieObject.bdm"] {
        let path = bdmv.join(name);
        if path.is_file() {
            members.push(RecognizedMember::new(path, AVCHD_NAVIGATION_ROLE, false));
        }
    }
    if members.iter().any(RecognizedMember::is_required) {
        Ok(Some(RecognizedMedia::Package(members)))
    } else {
        Ok(None)
    }
}

fn append_role_files(
    members: &mut Vec<RecognizedMember>,
    directory: &Path,
    role: &str,
    required: bool,
) -> Result<()> {
    if directory.is_dir() {
        members.extend(
            sorted_files(directory)?
                .into_iter()
                .map(|path| RecognizedMember::new(path, role, required)),
        );
    }
    Ok(())
}

/// The prefix, suffix, and digit count shared by numbered file names.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct NumberedGroup {
    pub(crate) prefix: String,
    pub(crate) suffix: String,
    pub(crate) padding: u8,
}

fn recognize_numbered_groups(directory: &Path, rate: RationalRate) -> Result<Vec<RecognizedMedia>> {
    let mut groups: BTreeMap<NumberedGroup, Vec<(i64, PathBuf)>> = BTreeMap::new();
    for path in sorted_files(directory)? {
        let Some((group, number)) = numbered_name(&path) else {
            continue;
        };
        groups.entry(group).or_default().push((number, path));
    }
    let mut recognized = Vec::new();
    for (group, mut members) in groups {
        if members.len() < 2 {
            continue;
        }
        members.sort_by_key(|(number, _)| *number);
        if is_image_suffix(&group.suffix) {
            recognized.push(recognize_sequence(directory, group, &members, rate)?);
        } else if is_recording_suffix(&group.suffix) {
            recognized.push(RecognizedMedia::OrderedParts(
                members
                    .into_iter()
                    .map(|(_, path)| RecognizedMember::new(path, SPAN_PART_ROLE, true))
                    .collect(),
            ));
        }
    }
    Ok(recognized)
}

fn recognize_sequence(
    directory: &Path,
    group: NumberedGroup,
    members: &[(i64, PathBuf)],
    rate: RationalRate,
) -> Result<RecognizedMedia> {
    let start = members.first().map_or(0, |(number, _)| *number);
    let end = members.last().map_or(start, |(number, _)| *number);
    let present = members
        .iter()
        .map(|(number, _)| *number)
        .collect::<std::collections::BTreeSet<_>>();
    let missing_frames = (start..=end)
        .filter(|frame| !present.contains(frame))
        .collect();
    Ok(RecognizedMedia::ImageSequence {
        directory: directory.to_path_buf(),
        naming: SequenceNaming::new(group.prefix, group.suffix, group.padding)?,
        frames: FrameRange::new(start, end, 1)?,
        missing_frames,
        rate,
    })
}

/// Splits a file name such as `shot_0001.png` into its numbered group and
/// frame number: the last run of digits before the final extension.
pub(crate) fn numbered_name(path: &Path) -> Option<(NumberedGroup, i64)> {
    let name = path.file_name()?.to_str()?;
    let suffix_start = name.rfind('.')?;
    let suffix = &name[suffix_start..];
    let stem = &name[..suffix_start];
    let digit_start = stem
        .trim_end_matches(|character: char| character.is_ascii_digit())
        .len();
    let digits = &stem[digit_start..];
    if digits.is_empty() || digits.len() > usize::from(u8::MAX) {
        return None;
    }
    let number = digits.parse().ok()?;
    Some((
        NumberedGroup {
            prefix: stem[..digit_start].to_owned(),
            suffix: suffix.to_owned(),
            padding: u8::try_from(digits.len()).ok()?,
        },
        number,
    ))
}

fn is_image_suffix(suffix: &str) -> bool {
    matches!(
        suffix.to_ascii_lowercase().as_str(),
        ".exr" | ".dpx" | ".tif" | ".tiff"
    )
}

fn is_recording_suffix(suffix: &str) -> bool {
    matches!(
        suffix.to_ascii_lowercase().as_str(),
        ".mov" | ".mxf" | ".mp4" | ".mts"
    )
}

fn sorted_files(directory: &Path) -> Result<Vec<PathBuf>> {
    let mut files = fs::read_dir(directory)
        .map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("list {}: {error}", directory.display()),
            )
        })?
        .filter_map(std::result::Result::ok)
        .filter_map(|entry| {
            entry
                .file_type()
                .ok()
                .filter(fs::FileType::is_file)
                .map(|_| entry.path())
        })
        .collect::<Vec<_>>();
    files.sort();
    Ok(files)
}
