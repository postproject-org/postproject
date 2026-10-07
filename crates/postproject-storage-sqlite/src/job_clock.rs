//! Private clock authority; callers cannot select lease time.

use std::{fmt::Debug, path::PathBuf, sync::Arc};

use postproject_core::{Error, ErrorKind, Result, Timestamp};
use rusqlite::{Connection, params};

use crate::sqlite_error;

pub(crate) trait JobClock: Debug + Send + Sync {
    fn now(&self) -> Result<Timestamp>;
}

pub(crate) struct LeaseAuthority {
    pub clock: Arc<dyn JobClock>,
    pub path: PathBuf,
}

#[derive(Debug)]
struct SystemClock;

impl JobClock for SystemClock {
    fn now(&self) -> Result<Timestamp> {
        Timestamp::now()
    }
}

pub(crate) fn system_clock() -> Arc<dyn JobClock> {
    Arc::new(SystemClock)
}

pub(crate) fn observe(connection: &Connection, now: Timestamp) -> Result<()> {
    let previous: Option<i64> = connection
        .query_row(
            "SELECT high_water_micros FROM job_clock WHERE singleton = 1",
            [],
            |row| row.get(0),
        )
        .map_err(sqlite_error("read job clock authority"))?;
    if previous.is_some_and(|previous| now.as_unix_micros() < previous) {
        return Err(Error::new(
            ErrorKind::Conflict,
            "job authority clock moved backward",
        ));
    }
    persist(connection, now.as_unix_micros())
}

pub(crate) fn persist(connection: &Connection, high_water: i64) -> Result<()> {
    connection
        .execute(
            "UPDATE job_clock SET high_water_micros =
                CASE WHEN high_water_micros IS NULL OR high_water_micros < ?1
                     THEN ?1 ELSE high_water_micros END
             WHERE singleton = 1",
            params![high_water],
        )
        .map_err(sqlite_error("persist job clock authority"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn high_water_rejects_rollback_and_retains_forward_observations() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                include_str!("migrations/019_job_clock.sql")
                    .split("-- Expire")
                    .next()
                    .unwrap(),
            )
            .unwrap();
        observe(&connection, Timestamp::from_unix_micros(100)).unwrap();
        observe(&connection, Timestamp::from_unix_micros(100)).unwrap();
        observe(&connection, Timestamp::from_unix_micros(200)).unwrap();
        assert_eq!(
            observe(&connection, Timestamp::from_unix_micros(199))
                .unwrap_err()
                .kind(),
            ErrorKind::Conflict
        );
        persist(&connection, 50).unwrap();
        assert!(observe(&connection, Timestamp::from_unix_micros(199)).is_err());
    }
}
