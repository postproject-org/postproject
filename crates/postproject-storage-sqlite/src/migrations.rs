//! Numbered, transactional SQLite schema migrations.

#[cfg(test)]
#[path = "migrations/job_observations.rs"]
mod job_observations;
#[cfg(test)]
#[path = "migrations/outcome_bounds.rs"]
mod outcome_bounds;

use postproject_core::{Error, ErrorKind, Result, Timestamp};
use rusqlite::{Connection, Transaction, TransactionBehavior, params};

/// The newest schema understood by this build.
pub const CURRENT_SCHEMA_VERSION: u32 = 26;

struct Migration {
    version: u32,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        sql: include_str!("migrations/001_initial.sql"),
    },
    Migration {
        version: 2,
        sql: include_str!("migrations/002_activities.sql"),
    },
    Migration {
        version: 3,
        sql: include_str!("migrations/003_revisions.sql"),
    },
    Migration {
        version: 4,
        sql: include_str!("migrations/004_activity_identifiers.sql"),
    },
    Migration {
        version: 5,
        sql: include_str!("migrations/005_lifecycle_events.sql"),
    },
    Migration {
        version: 6,
        sql: include_str!("migrations/006_portable_media_roots.sql"),
    },
    Migration {
        version: 7,
        sql: include_str!("migrations/007_fingerprint_observations.sql"),
    },
    Migration {
        version: 8,
        sql: include_str!("migrations/008_dependencies.sql"),
    },
    Migration {
        version: 9,
        sql: include_str!("migrations/009_jobs.sql"),
    },
    Migration {
        version: 10,
        sql: include_str!("migrations/010_job_query_indexes.sql"),
    },
    Migration {
        version: 11,
        sql: include_str!("migrations/011_domain_query_indexes.sql"),
    },
    Migration {
        version: 12,
        sql: include_str!("migrations/012_query_support.sql"),
    },
    Migration {
        version: 13,
        sql: include_str!("migrations/013_revision_event_kinds.sql"),
    },
    Migration {
        version: 14,
        sql: include_str!("migrations/014_job_completion_index.sql"),
    },
    Migration {
        version: 15,
        sql: include_str!("migrations/015_locator_sequence_namings.sql"),
    },
    Migration {
        version: 16,
        sql: include_str!("migrations/016_known_media_lookup.sql"),
    },
    Migration {
        version: 17,
        sql: include_str!("migrations/017_conflict_versions.sql"),
    },
    Migration {
        version: 18,
        sql: include_str!("migrations/018_file_fact_events.sql"),
    },
    Migration {
        version: 19,
        sql: include_str!("migrations/019_job_clock.sql"),
    },
    Migration {
        version: 20,
        sql: include_str!("migrations/020_exchange_history.sql"),
    },
    Migration {
        version: 21,
        sql: include_str!("migrations/021_exchange_effect_fragments.sql"),
    },
    Migration {
        version: 22,
        sql: include_str!("migrations/022_exchange_outcomes.sql"),
    },
    Migration {
        version: 23,
        sql: include_str!("migrations/023_exchange_records.sql"),
    },
    Migration {
        version: 24,
        sql: include_str!("migrations/024_exchange_chunk_fragments.sql"),
    },
    Migration {
        version: 25,
        sql: include_str!("migrations/025_inert_job_observations.sql"),
    },
    Migration {
        version: 26,
        sql: include_str!("migrations/026_job_result_outcomes.sql"),
    },
];

pub(crate) fn migrate(connection: &mut Connection) -> Result<()> {
    let current = schema_version(connection)?;
    if current > CURRENT_SCHEMA_VERSION {
        return Err(Error::new(
            ErrorKind::Unsupported,
            format!(
                "production schema version {current} is newer than supported version \
                 {CURRENT_SCHEMA_VERSION}"
            ),
        ));
    }

    for migration in MIGRATIONS {
        if migration.version > current {
            apply_migration(connection, migration)?;
        }
    }
    Ok(())
}

fn schema_version(connection: &Connection) -> Result<u32> {
    connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|error| {
            Error::new(
                ErrorKind::Migration,
                format!("read production schema version: {error}"),
            )
        })
}

fn apply_migration(connection: &mut Connection, migration: &Migration) -> Result<()> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(migration_error(migration.version, "begin"))?;
    apply_migration_in(&transaction, migration)?;
    transaction
        .commit()
        .map_err(migration_error(migration.version, "commit"))
}

fn apply_migration_in(transaction: &Transaction<'_>, migration: &Migration) -> Result<()> {
    transaction
        .execute_batch(migration.sql)
        .map_err(migration_error(migration.version, "apply statements"))?;
    if matches!(migration.version, 20 | 23) {
        crate::exchange::initialize_anchor(transaction)?;
    }
    transaction
        .execute(
            "INSERT INTO schema_migrations (version, applied_at_micros) VALUES (?1, ?2)",
            params![
                migration.version,
                Timestamp::now()
                    .map_err(|error| Error::new(ErrorKind::Migration, error.to_string()))?
                    .as_unix_micros()
            ],
        )
        .map_err(migration_error(migration.version, "record history"))?;
    transaction
        .pragma_update(None, "user_version", migration.version)
        .map_err(migration_error(migration.version, "record schema version"))?;
    Ok(())
}

fn migration_error(version: u32, action: &'static str) -> impl FnOnce(rusqlite::Error) -> Error {
    move |error| {
        Error::new(
            ErrorKind::Migration,
            format!("migration {version}: {action}: {error}"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use postproject_core::{
        ArtifactEvaluationLimits, ArtifactKnowledgeReason, ArtifactKnowledgeState, LocatorIdentity,
        QueryPageRequest, RepresentationId, ResourceFingerprint,
    };

    use crate::{SqliteProduction, load_production};

    #[test]
    fn schema_twenty_three_chunks_migrate_without_changing_exact_bytes() {
        let mut connection = Connection::open_in_memory().unwrap();
        for migration in &MIGRATIONS[..23] {
            apply_migration(&mut connection, migration).unwrap();
        }
        connection.execute("INSERT INTO revisions (id, sequence, transaction_id, committed_at_micros) VALUES (zeroblob(16), 1, zeroblob(16), 0)", []).unwrap();
        let bytes = vec![42_u8; 1_048_577];
        connection.execute("INSERT INTO exchange_record_chunks (revision_id, position, document) VALUES (zeroblob(16), 0, ?1)", [&bytes]).unwrap();
        migrate(&mut connection).unwrap();
        let fragments: Vec<(i64, Vec<u8>)> = connection.prepare("SELECT fragment_position, document FROM exchange_record_chunks ORDER BY fragment_position").unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?))).unwrap().collect::<std::result::Result<_, _>>().unwrap();
        assert_eq!(fragments.len(), 2);
        assert_eq!(fragments[0].0, 0);
        assert_eq!(fragments[1].0, 1);
        assert_eq!(fragments[0].1.len(), 1_048_576);
        assert_eq!(fragments[1].1.len(), 1);
        assert_eq!(
            [fragments[0].1.as_slice(), fragments[1].1.as_slice()].concat(),
            bytes
        );
    }

    #[test]
    fn migrates_schema_zero_fixture_to_current() {
        let mut connection = Connection::open_in_memory().expect("open in-memory database");
        connection
            .execute_batch(include_str!("../../../tests/fixtures/schema-000.sql"))
            .expect("load schema-zero fixture");

        migrate(&mut connection).expect("migrate schema-zero fixture");

        assert_eq!(
            schema_version(&connection).expect("read migrated version"),
            CURRENT_SCHEMA_VERSION
        );
        let applied: Vec<u32> = connection
            .prepare("SELECT version FROM schema_migrations ORDER BY version")
            .expect("prepare migration-history query")
            .query_map([], |row| row.get(0))
            .expect("query migration history")
            .collect::<std::result::Result<_, _>>()
            .expect("read migration history");
        assert_eq!(applied, (1..=CURRENT_SCHEMA_VERSION).collect::<Vec<_>>());
        for table in [
            "productions",
            "assets",
            "representations",
            "resources",
            "representation_resources",
            "image_sequences",
            "image_sequence_missing_frames",
            "resource_fingerprints",
            "representation_fingerprints",
            "locators",
            "locator_sequence_namings",
            "media_roots",
            "external_identifiers",
            "metadata_assertions",
            "activities",
            "activity_inputs",
            "activity_outputs",
            "revisions",
            "revision_events",
            "resource_fingerprint_history",
            "representation_fingerprint_history",
            "representation_fingerprint_recomputations",
            "activity_input_fingerprint_snapshots",
            "activity_output_fingerprint_snapshots",
            "dependency_sets",
            "dependencies",
            "activity_input_dependency_snapshots",
            "activity_input_dependency_paths",
            "activity_input_dependency_path_edges",
            "activity_input_dependency_fingerprint_snapshots",
            "jobs",
            "job_inputs",
            "unresolved_memberships",
            "activity_output_keys",
            "revision_event_kinds",
            "conflict_versions",
            "conflict_migration_baseline",
        ] {
            let count: u32 = connection
                .query_row(
                    "SELECT count(*) FROM sqlite_schema WHERE type = 'table' AND name = ?1",
                    [table],
                    |row| row.get(0),
                )
                .expect("query migrated table");
            assert_eq!(count, 1, "missing migrated table {table}");
        }
    }

    #[test]
    fn schema_fifteen_fixture_gains_indexed_known_media_lookup() {
        let mut connection = Connection::open_in_memory().expect("open in-memory database");
        for migration in &MIGRATIONS[..15] {
            apply_migration(&mut connection, migration).expect("apply released migration");
        }
        connection
            .execute_batch(include_str!(
                "../../../tests/fixtures/schema-015-known-media.sql"
            ))
            .expect("load schema-fifteen fixture");

        migrate(&mut connection).expect("migrate schema-fifteen fixture");

        for index in [
            "locators_by_uri_resource",
            "resource_fingerprints_by_value_resource",
        ] {
            let count: u32 = connection
                .query_row(
                    "SELECT count(*) FROM sqlite_schema WHERE type = 'index' AND name = ?1",
                    [index],
                    |row| row.get(0),
                )
                .expect("inspect lookup index");
            assert_eq!(count, 1, "missing {index}");
        }

        let production = load_production(&connection).expect("load migrated production");
        let production =
            SqliteProduction::from_parts(std::path::PathBuf::new(), connection, production)
                .expect("open migrated production");
        let page = QueryPageRequest::new(10, None).expect("page request");
        let locator =
            LocatorIdentity::new("file:///schema-15/known.mov", None).expect("locator identity");
        let fingerprint =
            ResourceFingerprint::new("fixture-host", 9, b"fixture-fingerprint".to_vec())
                .expect("fingerprint");
        assert_eq!(
            production
                .find_known_media_by_locator(&locator, &page)
                .expect("find migrated locator")
                .items()
                .len(),
            1
        );
        assert_eq!(
            production
                .find_known_media_by_fingerprint(&fingerprint, &page)
                .expect("find migrated fingerprint")
                .items()
                .len(),
            1
        );
    }

    #[test]
    fn schema_seventeen_preserves_journal_and_indexes_for_file_fact_events() {
        let mut connection = Connection::open_in_memory().unwrap();
        for migration in &MIGRATIONS[..17] {
            apply_migration(&mut connection, migration).unwrap();
        }
        connection
            .execute_batch(
                "INSERT INTO revisions (id, sequence, transaction_id, committed_at_micros)
             VALUES (zeroblob(16), 1, zeroblob(16), 0);
             INSERT INTO revision_events (revision_id, position, kind, primary_id)
             VALUES (zeroblob(16), 0, 17, zeroblob(16));",
            )
            .unwrap();
        migrate(&mut connection).unwrap();
        let event: (u32, Vec<u8>) = connection
            .query_row("SELECT kind, primary_id FROM revision_events", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .unwrap();
        assert_eq!(event, (17, vec![0; 16]));
        connection
            .execute_batch(
                "INSERT INTO revision_events (revision_id, position, kind, primary_id)
             VALUES (zeroblob(16), 1, 27, zeroblob(16));",
            )
            .unwrap();
        let kinds = connection
            .prepare("SELECT kind FROM revision_event_kinds WHERE sequence = 1 ORDER BY kind")
            .unwrap()
            .query_map([], |row| row.get::<_, u32>(0))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(kinds, [17, 27]);
        for index in [
            "revision_events_by_primary_target",
            "revision_events_by_secondary_target",
        ] {
            assert_eq!(
                connection
                    .query_row(
                        "SELECT count(*) FROM sqlite_schema WHERE type = 'index' AND name = ?1",
                        [index],
                        |row| row.get::<_, u32>(0)
                    )
                    .unwrap(),
                1
            );
        }
    }

    #[test]
    fn schema_sixteen_records_latest_revision_as_conflict_baseline() {
        let mut connection = Connection::open_in_memory().expect("open in-memory database");
        for migration in &MIGRATIONS[..16] {
            apply_migration(&mut connection, migration).expect("apply released migration");
        }
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 INSERT INTO productions (
                    singleton, id, schema_version, created_at_micros, display_name
                 ) VALUES (1, zeroblob(16), 16, 0, NULL);
                 INSERT INTO revisions (
                    id, sequence, transaction_id, committed_at_micros
                 ) VALUES (
                    x'01010101010101010101010101010101', 1,
                    x'11111111111111111111111111111111', 0
                 );
                 INSERT INTO revisions (
                    id, sequence, transaction_id, committed_at_micros
                 ) VALUES (
                    x'02020202020202020202020202020202', 2,
                    x'12121212121212121212121212121212', 0
                 );",
            )
            .expect("insert schema-sixteen production and revisions");

        migrate(&mut connection).expect("migrate schema-sixteen production");

        let baseline: (Vec<u8>, i64) = connection
            .query_row(
                "SELECT revision_id, revision_sequence
                 FROM conflict_migration_baseline WHERE singleton = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read conflict migration baseline");
        assert_eq!(baseline, (vec![2_u8; 16], 2));
        let version_count: u32 = connection
            .query_row("SELECT count(*) FROM conflict_versions", [], |row| {
                row.get(0)
            })
            .expect("count conflict versions");
        assert_eq!(version_count, 0);
    }

    #[test]
    fn later_migrations_update_existing_production_version() {
        let mut connection = Connection::open_in_memory().expect("open in-memory database");
        apply_migration(&mut connection, &MIGRATIONS[0]).expect("apply first migration");
        connection
            .execute(
                "INSERT INTO productions (
                    singleton, id, schema_version, created_at_micros, display_name
                 ) VALUES (1, zeroblob(16), 1, 0, NULL)",
                [],
            )
            .expect("insert version-one production");
        connection
            .execute(
                "INSERT INTO media_roots (id, uri, label, priority, enabled)
                 VALUES (?1, 'file:///mnt/media', 'Camera originals', 5, 1)",
                [vec![7_u8; 16]],
            )
            .expect("insert absolute media root");

        migrate(&mut connection).expect("migrate existing production");

        let production_version: u32 = connection
            .query_row("SELECT schema_version FROM productions", [], |row| {
                row.get(0)
            })
            .expect("read production version");
        assert_eq!(production_version, CURRENT_SCHEMA_VERSION);
        let migrated_root: (String, Option<String>, Option<String>) = connection
            .query_row(
                "SELECT name, label, legacy_uri FROM media_roots",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("read migrated media root");
        assert_eq!(
            migrated_root,
            (
                "legacy-07070707070707070707070707070707".to_owned(),
                Some("Camera originals".to_owned()),
                Some("file:///mnt/media".to_owned()),
            )
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one migrated production verifies backfill and every maintaining trigger"
    )]
    fn schema_eleven_backfills_and_maintains_query_support() {
        let mut connection = Connection::open_in_memory().expect("open database");
        for migration in &MIGRATIONS[..11] {
            apply_migration(&mut connection, migration).expect("apply old migration");
        }
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 INSERT INTO productions (
                    singleton, id, schema_version, created_at_micros, display_name
                 ) VALUES (1, zeroblob(16), 11, 0, NULL);
                 INSERT INTO assets VALUES (x'01010101010101010101010101010101', 0, NULL, NULL);",
            )
            .expect("insert production and asset");
        for label in [2_u8, 3] {
            connection
                .execute(
                    "INSERT INTO representations VALUES (?1, ?2, 3, 0)",
                    params![vec![label; 16], vec![1_u8; 16]],
                )
                .expect("insert representation");
            connection
                .execute(
                    "INSERT INTO resources (id) VALUES (?1)",
                    [vec![label + 10; 16]],
                )
                .expect("insert resource");
            connection
                .execute(
                    "INSERT INTO representation_resources VALUES (?1, ?2, 0, NULL, 1)",
                    params![vec![label; 16], vec![label + 10; 16]],
                )
                .expect("insert membership");
        }
        connection
            .execute(
                "INSERT INTO media_roots (id, name, priority, enabled)
                 VALUES (?1, 'media', 0, 1)",
                [vec![30_u8; 16]],
            )
            .expect("insert media root");
        connection
            .execute(
                "INSERT INTO locators (id, resource_id, uri, availability, media_root_name)
                 VALUES (?1, ?2, 'file:///media/a.mov', 1, 'media')",
                params![vec![20_u8; 16], vec![12_u8; 16]],
            )
            .expect("insert locator");
        connection
            .execute_batch(
                "INSERT INTO activities (id, kind, tool_name)
                 VALUES (x'04040404040404040404040404040404', 'example:transcode', 'tool');
                 INSERT INTO activity_outputs (activity_id, representation_id, role)
                 VALUES (x'04040404040404040404040404040404',
                         x'03030303030303030303030303030303', 'a');",
            )
            .expect("insert activity");

        migrate(&mut connection).expect("migrate schema eleven");

        let unresolved = |connection: &Connection| -> Vec<Vec<u8>> {
            connection
                .prepare("SELECT representation_id FROM unresolved_memberships ORDER BY 1")
                .expect("prepare unresolved query")
                .query_map([], |row| row.get(0))
                .expect("query unresolved")
                .collect::<std::result::Result<_, _>>()
                .expect("read unresolved")
        };
        assert_eq!(unresolved(&connection), [vec![3_u8; 16]]);
        let keys: (String, Option<String>) = connection
            .query_row(
                "SELECT kind, tool_name FROM activity_output_keys",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read migrated output key");
        assert_eq!(
            keys,
            ("example:transcode".to_owned(), Some("tool".to_owned()))
        );
        let rooted = |connection: &Connection| -> Vec<(String, Vec<u8>)> {
            connection
                .prepare(
                    "SELECT media_root_name, representation_id
                     FROM media_root_representations ORDER BY 1, 2",
                )
                .expect("prepare rooted query")
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .expect("query rooted")
                .collect::<std::result::Result<_, _>>()
                .expect("read rooted")
        };
        assert_eq!(rooted(&connection), [("media".to_owned(), vec![2_u8; 16])]);
        connection
            .execute("UPDATE media_roots SET name = 'renamed'", [])
            .expect("rename media root");
        assert_eq!(
            rooted(&connection),
            [("renamed".to_owned(), vec![2_u8; 16])]
        );

        connection
            .execute(
                "INSERT INTO locators (id, resource_id, uri, availability)
                 VALUES (?1, ?2, 'file:///media/b.mov', 1)",
                params![vec![21_u8; 16], vec![13_u8; 16]],
            )
            .expect("locate second resource");
        assert_eq!(unresolved(&connection), Vec::<Vec<u8>>::new());
        connection
            .execute("DELETE FROM locators WHERE id = ?1", [vec![20_u8; 16]])
            .expect("retire first locator");
        assert_eq!(unresolved(&connection), [vec![2_u8; 16]]);
        assert_eq!(rooted(&connection), []);

        connection
            .execute(
                "INSERT INTO activity_outputs (activity_id, representation_id, role)
                 VALUES (?1, ?2, 'b')",
                params![vec![4_u8; 16], vec![3_u8; 16]],
            )
            .expect("insert second output role");
        connection
            .execute("DELETE FROM activity_outputs WHERE role = 'a'", [])
            .expect("delete one output role");
        let key_count: u32 = connection
            .query_row("SELECT count(*) FROM activity_output_keys", [], |row| {
                row.get(0)
            })
            .expect("count output keys");
        assert_eq!(key_count, 1);
        connection
            .execute("DELETE FROM activity_outputs", [])
            .expect("delete remaining output");
        let key_count: u32 = connection
            .query_row("SELECT count(*) FROM activity_output_keys", [], |row| {
                row.get(0)
            })
            .expect("count output keys");
        assert_eq!(key_count, 0);
    }

    #[test]
    fn schema_twelve_backfills_and_maintains_revision_event_kinds() {
        let mut connection = Connection::open_in_memory().expect("open database");
        for migration in &MIGRATIONS[..12] {
            apply_migration(&mut connection, migration).expect("apply old migration");
        }
        let insert_revision = |connection: &Connection, label: u8, sequence: i64| {
            connection
                .execute(
                    "INSERT INTO revisions (id, sequence, transaction_id, committed_at_micros)
                     VALUES (?1, ?2, ?3, 0)",
                    params![vec![label; 16], sequence, vec![label + 100; 16]],
                )
                .expect("insert revision");
        };
        let insert_event = |connection: &Connection, label: u8, position: i64, kind: i64| {
            connection
                .execute(
                    "INSERT INTO revision_events (revision_id, position, kind, primary_id)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![vec![label; 16], position, kind, vec![label + 50; 16]],
                )
                .expect("insert revision event");
        };
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 INSERT INTO productions (
                    singleton, id, schema_version, created_at_micros, display_name
                 ) VALUES (1, zeroblob(16), 12, 0, NULL);",
            )
            .expect("insert production");
        insert_revision(&connection, 1, 1);
        insert_event(&connection, 1, 0, 1);
        insert_event(&connection, 1, 1, 3);
        insert_event(&connection, 1, 2, 3);
        insert_revision(&connection, 2, 2);
        insert_event(&connection, 2, 0, 20);

        migrate(&mut connection).expect("migrate schema twelve");

        let keys = |connection: &Connection| -> Vec<(i64, i64)> {
            connection
                .prepare("SELECT kind, sequence FROM revision_event_kinds ORDER BY 1, 2")
                .expect("prepare kind query")
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .expect("query kinds")
                .collect::<std::result::Result<_, _>>()
                .expect("read kinds")
        };
        assert_eq!(keys(&connection), [(1, 1), (3, 1), (20, 2)]);

        insert_revision(&connection, 3, 3);
        insert_event(&connection, 3, 0, 24);
        insert_event(&connection, 3, 1, 1);
        assert_eq!(
            keys(&connection),
            [(1, 1), (1, 3), (3, 1), (20, 2), (24, 3)]
        );
        connection
            .execute("DELETE FROM revisions WHERE sequence = 3", [])
            .expect("delete revision");
        assert_eq!(keys(&connection), [(1, 1), (3, 1), (20, 2)]);
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one migrated production verifies moved namings and every rebuilt table"
    )]
    fn schema_fourteen_moves_sequence_names_to_every_sequence_locator() {
        let mut connection = Connection::open_in_memory().expect("open database");
        for migration in &MIGRATIONS[..14] {
            apply_migration(&mut connection, migration).expect("apply old migration");
        }
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 INSERT INTO productions (
                    singleton, id, schema_version, created_at_micros, display_name
                 ) VALUES (1, zeroblob(16), 14, 0, NULL);
                 INSERT INTO assets VALUES (x'01010101010101010101010101010101', 0, NULL, NULL);
                 INSERT INTO media_roots (id, name, priority, enabled)
                 VALUES (x'1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e', 'plates', 0, 1);",
            )
            .expect("insert production");
        // Representation 2 is a sequence with two locators; representation 3 is
        // a single file with one.
        for label in [2_u8, 3] {
            connection
                .execute(
                    "INSERT INTO representations VALUES (?1, ?2, 1, 0)",
                    params![vec![label; 16], vec![1_u8; 16]],
                )
                .expect("insert representation");
            connection
                .execute(
                    "INSERT INTO resources (id) VALUES (?1)",
                    [vec![label + 10; 16]],
                )
                .expect("insert resource");
            connection
                .execute(
                    "INSERT INTO representation_resources VALUES (?1, ?2, 0, NULL, 1)",
                    params![vec![label; 16], vec![label + 10; 16]],
                )
                .expect("insert membership");
        }
        connection
            .execute(
                "INSERT INTO image_sequences (
                    representation_id, resource_id, prefix, suffix, padding,
                    start_frame, end_frame, frame_step, rate_numerator, rate_denominator
                 ) VALUES (?1, ?2, 'shot_', '.png', 4, 1, 3, 1, 24, 1)",
                params![vec![2_u8; 16], vec![12_u8; 16]],
            )
            .expect("insert sequence");
        connection
            .execute(
                "INSERT INTO image_sequence_missing_frames VALUES (?1, 2)",
                [vec![2_u8; 16]],
            )
            .expect("insert missing frame");
        for (label, resource, uri, root) in [
            (20_u8, 12_u8, "file:///plates/", Some("plates")),
            (21, 12, "file:///backup/", None),
            (22, 13, "file:///media/a.mov", None),
        ] {
            connection
                .execute(
                    "INSERT INTO locators (id, resource_id, uri, availability, media_root_name)
                     VALUES (?1, ?2, ?3, 1, ?4)",
                    params![vec![label; 16], vec![resource; 16], uri, root],
                )
                .expect("insert locator");
        }

        migrate(&mut connection).expect("migrate schema fourteen");

        let namings = |connection: &Connection| -> Vec<(Vec<u8>, String, String, u8)> {
            connection
                .prepare(
                    "SELECT locator_id, prefix, suffix, padding
                     FROM locator_sequence_namings ORDER BY 1",
                )
                .expect("prepare naming query")
                .query_map([], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
                })
                .expect("query namings")
                .collect::<std::result::Result<_, _>>()
                .expect("read namings")
        };
        assert_eq!(
            namings(&connection),
            [
                (vec![20_u8; 16], "shot_".to_owned(), ".png".to_owned(), 4),
                (vec![21_u8; 16], "shot_".to_owned(), ".png".to_owned(), 4),
            ]
        );
        let sequence_columns: Vec<String> = connection
            .prepare("SELECT name FROM pragma_table_info('image_sequences') ORDER BY cid")
            .expect("prepare column query")
            .query_map([], |row| row.get(0))
            .expect("query columns")
            .collect::<std::result::Result<_, _>>()
            .expect("read columns");
        assert_eq!(
            sequence_columns,
            [
                "representation_id",
                "resource_id",
                "start_frame",
                "end_frame",
                "frame_step",
                "rate_numerator",
                "rate_denominator"
            ]
        );
        let missing: i64 = connection
            .query_row(
                "SELECT frame FROM image_sequence_missing_frames",
                [],
                |row| row.get(0),
            )
            .expect("read missing frame");
        assert_eq!(missing, 2);
        let rooted: u32 = connection
            .query_row(
                "SELECT count(*) FROM media_root_representations WHERE locator_id = ?1",
                [vec![20_u8; 16]],
                |row| row.get(0),
            )
            .expect("count rooted locator");
        assert_eq!(rooted, 1);
        let violations: u32 = connection
            .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })
            .expect("check foreign keys");
        assert_eq!(violations, 0);

        // The rebuilt locator table keeps its maintaining triggers, lets a
        // resource hold one URI under two namings, and cascades to namings.
        connection
            .execute(
                "INSERT INTO locators (id, resource_id, uri, availability)
                 VALUES (?1, ?2, 'file:///plates/', 1)",
                params![vec![23_u8; 16], vec![12_u8; 16]],
            )
            .expect("add second locator at one URI");
        connection
            .execute(
                "INSERT INTO locator_sequence_namings VALUES (?1, 'shot-graded_', '.png', 4)",
                [vec![23_u8; 16]],
            )
            .expect("record second naming");
        connection
            .execute(
                "DELETE FROM locators WHERE resource_id = ?1",
                [vec![12_u8; 16]],
            )
            .expect("retire sequence locators");
        assert_eq!(namings(&connection), []);
        let unresolved: u32 = connection
            .query_row("SELECT count(*) FROM unresolved_memberships", [], |row| {
                row.get(0)
            })
            .expect("count unresolved");
        assert_eq!(unresolved, 1);
        connection
            .execute(
                "DELETE FROM representations WHERE id = ?1",
                [vec![2_u8; 16]],
            )
            .expect("delete sequence representation");
        let remaining: u32 = connection
            .query_row(
                "SELECT count(*) FROM image_sequence_missing_frames",
                [],
                |row| row.get(0),
            )
            .expect("count missing frames");
        assert_eq!(remaining, 0);
    }

    #[test]
    fn schema_six_activities_migrate_without_fabricated_snapshots() {
        let mut connection = Connection::open_in_memory().expect("open database");
        for migration in &MIGRATIONS[..6] {
            apply_migration(&mut connection, migration).expect("apply old migration");
        }
        connection
            .execute(
                "INSERT INTO productions (
                    singleton, id, schema_version, created_at_micros, display_name
                 ) VALUES (1, zeroblob(16), 6, 0, NULL)",
                [],
            )
            .expect("insert production");
        connection
            .execute(
                "INSERT INTO assets VALUES (?1, 0, NULL, NULL)",
                [vec![1_u8; 16]],
            )
            .expect("insert asset");
        for label in [2_u8, 3] {
            connection
                .execute(
                    "INSERT INTO representations VALUES (?1, ?2, 3, 0)",
                    params![vec![label; 16], vec![1_u8; 16]],
                )
                .expect("insert representation");
            connection
                .execute(
                    "INSERT INTO resources (id) VALUES (?1)",
                    [vec![label + 10; 16]],
                )
                .expect("insert resource");
            connection
                .execute(
                    "INSERT INTO representation_resources VALUES (?1, ?2, 0, NULL, 1)",
                    params![vec![label; 16], vec![label + 10; 16]],
                )
                .expect("insert representation resource");
        }
        connection
            .execute(
                "INSERT INTO activities (id, kind) VALUES (?1, 'example:activity')",
                [vec![4_u8; 16]],
            )
            .expect("insert activity");
        connection
            .execute(
                "INSERT INTO activity_inputs (activity_id, representation_id)
                 VALUES (?1, ?2)",
                params![vec![4_u8; 16], vec![2_u8; 16]],
            )
            .expect("insert input");
        connection
            .execute(
                "INSERT INTO activity_outputs (activity_id, representation_id)
                 VALUES (?1, ?2)",
                params![vec![4_u8; 16], vec![3_u8; 16]],
            )
            .expect("insert output");

        migrate(&mut connection).expect("migrate schema six");

        for table in ["activity_inputs", "activity_outputs"] {
            let snapshot: Option<i64> = connection
                .query_row(
                    &format!("SELECT snapshot_revision_sequence FROM {table}"),
                    [],
                    |row| row.get(0),
                )
                .expect("load migrated snapshot");
            assert_eq!(snapshot, None);
        }
        let dependency_snapshot_count: u32 = connection
            .query_row(
                "SELECT count(*) FROM activity_input_dependency_snapshots",
                [],
                |row| row.get(0),
            )
            .expect("count migrated dependency snapshots");
        assert_eq!(dependency_snapshot_count, 0);

        let production = load_production(&connection).expect("load migrated production");
        let production =
            SqliteProduction::from_parts(std::path::PathBuf::new(), connection, production)
                .expect("open migrated production");
        let evaluation = production
            .evaluate_artifact(
                RepresentationId::from_bytes([3_u8; 16]),
                ArtifactEvaluationLimits::default(),
            )
            .expect("evaluate migrated activity");
        assert_eq!(evaluation.state(), ArtifactKnowledgeState::Indeterminate);
        assert!(
            evaluation
                .reasons()
                .iter()
                .any(|reason| matches!(reason, ArtifactKnowledgeReason::SnapshotAbsent { .. }))
        );
    }

    #[test]
    fn newer_schema_is_rejected_without_modification() {
        let mut connection = Connection::open_in_memory().expect("open in-memory database");
        connection
            .pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION + 1)
            .expect("set future schema version");

        let error = migrate(&mut connection).expect_err("future schema must be rejected");

        assert_eq!(error.kind(), ErrorKind::Unsupported);
        assert_eq!(
            schema_version(&connection).expect("read unchanged version"),
            CURRENT_SCHEMA_VERSION + 1
        );
    }

    #[test]
    fn failed_migration_rolls_back_completely() {
        let mut connection = Connection::open_in_memory().expect("open in-memory database");
        let invalid = Migration {
            version: 1,
            sql: "CREATE TABLE partial (value INTEGER); INVALID SQL;",
        };

        let error = apply_migration(&mut connection, &invalid)
            .expect_err("invalid migration must be rejected");

        assert_eq!(error.kind(), ErrorKind::Migration);
        assert_eq!(schema_version(&connection).expect("read version"), 0);
        let table_count: u32 = connection
            .query_row(
                "SELECT count(*) FROM sqlite_schema WHERE name = 'partial'",
                [],
                |row| row.get(0),
            )
            .expect("query schema");
        assert_eq!(table_count, 0);
    }
}
