use std::{fmt::Display, str::FromStr};

use postproject_core::{
    DependencyQueryLimits, DependencyTarget, Error, ErrorKind, JobId, JobQuery, QueryCursor,
    QueryPageRequest, RepresentationId, Result,
};

const CURSOR_VERSION: &str = "ppq1";

pub(crate) fn signature(parts: &[&[u8]]) -> String {
    let mut hasher = blake3::Hasher::new();
    for part in parts {
        hasher.update(&(part.len() as u64).to_be_bytes());
        hasher.update(part);
    }
    hasher.finalize().to_hex().to_string()
}

pub(crate) fn position_fields<'a>(
    page: &'a QueryPageRequest,
    query: &str,
    query_signature: &str,
    key_fields: usize,
) -> Result<Option<Vec<&'a str>>> {
    let Some(cursor) = page.cursor() else {
        return Ok(None);
    };
    let fields = cursor.as_str().split('|').collect::<Vec<_>>();
    if fields.len() != key_fields + 3
        || fields[0] != CURSOR_VERSION
        || fields[1] != query
        || fields[2] != query_signature
    {
        return Err(invalid_cursor());
    }
    Ok(Some(fields.into_iter().skip(3).collect()))
}

pub(crate) fn cursor(
    query: &str,
    query_signature: &str,
    key_fields: &[String],
) -> Result<QueryCursor> {
    QueryCursor::new(format!(
        "{CURSOR_VERSION}|{query}|{query_signature}|{}",
        key_fields.join("|")
    ))
}

pub(crate) fn id_position<T>(
    page: &QueryPageRequest,
    query: &str,
    query_signature: &str,
) -> Result<Option<[u8; 16]>>
where
    T: FromStr<Err = Error> + Display,
    T: IntoIdBytes,
{
    position_fields(page, query, query_signature, 1)?
        .map(|fields| parse_id::<T>(fields[0]).map(IntoIdBytes::into_id_bytes))
        .transpose()
}

pub(crate) fn id_pair_position<A, B>(
    page: &QueryPageRequest,
    query: &str,
    query_signature: &str,
) -> Result<Option<([u8; 16], [u8; 16])>>
where
    A: FromStr<Err = Error> + Display + IntoIdBytes,
    B: FromStr<Err = Error> + Display + IntoIdBytes,
{
    position_fields(page, query, query_signature, 2)?
        .map(|fields| {
            Ok((
                parse_id::<A>(fields[0])?.into_id_bytes(),
                parse_id::<B>(fields[1])?.into_id_bytes(),
            ))
        })
        .transpose()
}

pub(crate) trait IntoIdBytes {
    fn into_id_bytes(self) -> [u8; 16];
}

macro_rules! id_bytes {
    ($($id:ty),+ $(,)?) => {
        $(impl IntoIdBytes for $id {
            fn into_id_bytes(self) -> [u8; 16] { self.into_bytes() }
        })+
    };
}

id_bytes!(
    postproject_core::ActivityId,
    postproject_core::AssetId,
    postproject_core::LocatorId,
    postproject_core::RepresentationId,
    postproject_core::ResourceId,
);

pub(crate) fn dependency_position(
    page: &QueryPageRequest,
    source: RepresentationId,
    limits: DependencyQueryLimits,
) -> Result<Option<(u8, [u8; 16])>> {
    let Some(cursor) = page.cursor() else {
        return Ok(None);
    };
    let fields = cursor.as_str().split('|').collect::<Vec<_>>();
    if fields.len() != 7
        || fields[0] != CURSOR_VERSION
        || fields[1] != "dependencies"
        || fields[2] != source.to_string()
        || fields[3] != limits.max_depth().to_string()
        || fields[4] != limits.max_representations().to_string()
    {
        return Err(invalid_cursor());
    }
    let kind = parse_target_kind(fields[5])?;
    let id = parse_id::<RepresentationId>(fields[6])?.into_bytes();
    Ok(Some((kind, id)))
}

pub(crate) fn dependency_cursor(
    source: RepresentationId,
    limits: DependencyQueryLimits,
    key: (u8, [u8; 16]),
) -> Result<QueryCursor> {
    QueryCursor::new(format!(
        "{CURSOR_VERSION}|dependencies|{source}|{}|{}|{}|{}",
        limits.max_depth(),
        limits.max_representations(),
        key.0,
        RepresentationId::from_bytes(key.1),
    ))
}

pub(crate) fn dependent_position(
    page: &QueryPageRequest,
    target: DependencyTarget,
    limits: DependencyQueryLimits,
) -> Result<Option<[u8; 16]>> {
    let Some(cursor) = page.cursor() else {
        return Ok(None);
    };
    let fields = cursor.as_str().split('|').collect::<Vec<_>>();
    let (target_kind, target_id) = target_parts(target);
    if fields.len() != 7
        || fields[0] != CURSOR_VERSION
        || fields[1] != "dependents"
        || fields[2] != target_kind.to_string()
        || fields[3] != target_id
        || fields[4] != limits.max_depth().to_string()
        || fields[5] != limits.max_representations().to_string()
    {
        return Err(invalid_cursor());
    }
    Ok(Some(parse_id::<RepresentationId>(fields[6])?.into_bytes()))
}

pub(crate) fn dependent_cursor(
    target: DependencyTarget,
    limits: DependencyQueryLimits,
    id: [u8; 16],
) -> Result<QueryCursor> {
    let (target_kind, target_id) = target_parts(target);
    QueryCursor::new(format!(
        "{CURSOR_VERSION}|dependents|{target_kind}|{target_id}|{}|{}|{}",
        limits.max_depth(),
        limits.max_representations(),
        RepresentationId::from_bytes(id),
    ))
}

pub(crate) fn job_position(page: &QueryPageRequest, query: &JobQuery) -> Result<Option<[u8; 16]>> {
    let Some(cursor) = page.cursor() else {
        return Ok(None);
    };
    let fields = cursor.as_str().split('|').collect::<Vec<_>>();
    let state = query.state().map_or(0, job_state_code);
    let kind = query.kind().map_or("*", |kind| kind.as_str());
    if fields.len() != 5
        || fields[0] != CURSOR_VERSION
        || fields[1] != "jobs"
        || fields[2] != state.to_string()
        || fields[3] != kind
    {
        return Err(invalid_cursor());
    }
    Ok(Some(parse_id::<JobId>(fields[4])?.into_bytes()))
}

pub(crate) fn job_cursor(query: &JobQuery, id: [u8; 16]) -> Result<QueryCursor> {
    let state = query.state().map_or(0, job_state_code);
    let kind = query.kind().map_or("*", |kind| kind.as_str());
    QueryCursor::new(format!(
        "{CURSOR_VERSION}|jobs|{state}|{kind}|{}",
        JobId::from_bytes(id),
    ))
}

fn target_parts(target: DependencyTarget) -> (u8, String) {
    match target {
        DependencyTarget::Asset(id) => (1, id.to_string()),
        DependencyTarget::Representation(id) => (2, id.to_string()),
        _ => (0, String::new()),
    }
}

fn parse_target_kind(value: &str) -> Result<u8> {
    match value {
        "1" => Ok(1),
        "2" => Ok(2),
        _ => Err(invalid_cursor()),
    }
}

fn parse_id<T>(value: &str) -> Result<T>
where
    T: FromStr<Err = Error> + Display,
{
    let id = value.parse::<T>().map_err(|_| invalid_cursor())?;
    if id.to_string() != value {
        return Err(invalid_cursor());
    }
    Ok(id)
}

pub(crate) const fn job_state_code(state: postproject_core::JobStateKind) -> u8 {
    match state {
        postproject_core::JobStateKind::Requested => 1,
        postproject_core::JobStateKind::Claimed => 2,
        postproject_core::JobStateKind::Succeeded => 3,
        postproject_core::JobStateKind::Failed => 4,
        postproject_core::JobStateKind::Cancelled => 5,
        _ => u8::MAX,
    }
}

fn invalid_cursor() -> Error {
    Error::new(
        ErrorKind::InvalidArgument,
        "query cursor does not match this query and its parameters",
    )
}
