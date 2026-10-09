//! Parent replacement preserves enforced children and private authority detail.

use postproject_core::{JobId, JobState};
use rusqlite::{Connection, types::Value};

use super::{MIGRATIONS, apply_migration, migrate, schema_version};
use crate::{SqliteProduction, load_production};

fn fixture() -> Connection {
    let mut connection = Connection::open_in_memory().unwrap();
    connection
        .pragma_update(None, "foreign_keys", true)
        .unwrap();
    for migration in &MIGRATIONS[..24] {
        apply_migration(&mut connection, migration).unwrap();
    }
    connection
        .execute_batch(
            "INSERT INTO productions VALUES (1, zeroblob(16), 24, 0, NULL);
        INSERT INTO assets VALUES (zeroblob(16), 0, NULL, NULL);
        INSERT INTO representations VALUES (zeroblob(16), zeroblob(16), 3, 0);
        INSERT INTO resources (id) VALUES (zeroblob(16));
        INSERT INTO representation_resources VALUES (zeroblob(16), zeroblob(16), 0, NULL, 1);
        INSERT INTO activities (id, kind) VALUES (zeroblob(16), 'example:generate');
        UPDATE job_clock SET high_water_micros = 123456;",
        )
        .unwrap();
    for state in 1_u8..=5 {
        let id = [state; 16];
        connection
            .execute(
                "INSERT INTO jobs (id, kind, output_asset_id, output_representation_kind, state,
            claim_id, claim_tool_name, claim_expires_at_micros,
            completion_activity_id, completion_representation_id, failure_diagnostic)
            VALUES (?1, 'example:job', zeroblob(16), 3, ?2,
                CASE WHEN ?2 = 2 THEN ?1 END, CASE WHEN ?2 = 2 THEN 'Exact tool' END,
                CASE WHEN ?2 = 2 THEN 9007199254740993 END,
                CASE WHEN ?2 = 3 THEN zeroblob(16) END,
                CASE WHEN ?2 = 3 THEN zeroblob(16) END,
                CASE WHEN ?2 = 4 THEN 'Exact failure' END)",
                (id.as_slice(), state),
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO job_inputs VALUES (?1, 0, zeroblob(16))",
                [id.as_slice()],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO metadata_assertions
            (target_kind, target_id, vocabulary, property, position, encoded_value)
            VALUES (5, ?1, 'urn:exact', 'Exact', 0, X'50504d560100')",
                [id.as_slice()],
            )
            .unwrap();
    }
    connection
}

fn rows(connection: &Connection, sql: &str) -> Vec<Vec<Value>> {
    let mut statement = connection.prepare(sql).unwrap();
    let count = statement.column_count();
    statement
        .query_map([], |row| (0..count).map(|index| row.get(index)).collect())
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

#[test]
fn parent_replacement_preserves_all_states_children_attachments_and_private_facts() {
    let mut connection = fixture();
    let queries = [
        "SELECT id, state, claim_id, claim_tool_name, claim_expires_at_micros, completion_activity_id, completion_representation_id, failure_diagnostic FROM jobs ORDER BY id",
        "SELECT * FROM job_inputs ORDER BY job_id, position",
        "SELECT * FROM metadata_assertions ORDER BY target_id",
        "SELECT * FROM job_clock",
        "SELECT * FROM exchange_history",
    ];
    let before = queries.map(|query| rows(&connection, query));
    migrate(&mut connection).unwrap();
    for (query, expected) in queries.iter().zip(before) {
        assert_eq!(rows(&connection, query), expected, "{query}");
    }
    assert_eq!(
        rows(&connection, "PRAGMA foreign_keys"),
        vec![vec![Value::Integer(1)]]
    );
    assert_eq!(
        rows(&connection, "PRAGMA foreign_key_check"),
        Vec::<Vec<Value>>::new()
    );
    assert_eq!(
        rows(&connection, "SELECT sum(claim_inert) FROM jobs"),
        vec![vec![Value::Integer(0)]]
    );
    for index in [
        "jobs_by_state_kind",
        "jobs_by_kind_id",
        "jobs_by_completion_activity",
        "job_inputs_by_representation",
    ] {
        assert_eq!(
            connection
                .query_row(
                    "SELECT count(*) FROM sqlite_schema WHERE type = 'index' AND name = ?1",
                    [index],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
    }
    connection
        .execute("DELETE FROM jobs WHERE id = ?1", [[1_u8; 16].as_slice()])
        .unwrap();
    assert_eq!(
        rows(&connection, "SELECT count(*) FROM job_inputs"),
        vec![vec![Value::Integer(4)]]
    );
    assert_eq!(
        rows(&connection, "SELECT count(*) FROM metadata_assertions"),
        vec![vec![Value::Integer(4)]]
    );
}

#[test]
fn a_late_child_restore_failure_rolls_back_the_entire_migration() {
    let mut connection = fixture();
    let before = rows(&connection, "SELECT * FROM jobs ORDER BY id");
    connection.execute_batch("CREATE TRIGGER reject_restored_input BEFORE INSERT ON job_inputs BEGIN SELECT RAISE(ABORT, 'injected'); END;").unwrap();
    assert!(migrate(&mut connection).is_err());
    assert_eq!(schema_version(&connection).unwrap(), 24);
    assert_eq!(rows(&connection, "SELECT * FROM jobs ORDER BY id"), before);
    assert_eq!(
        rows(&connection, "SELECT count(*) FROM job_inputs"),
        vec![vec![Value::Integer(5)]]
    );
    assert_eq!(
        rows(&connection, "SELECT count(*) FROM metadata_assertions"),
        vec![vec![Value::Integer(5)]]
    );
    assert_eq!(
        rows(
            &connection,
            "SELECT name FROM sqlite_schema WHERE name = 'jobs_exchange'"
        ),
        Vec::<Vec<Value>>::new()
    );
    assert_eq!(
        rows(
            &connection,
            "SELECT name FROM sqlite_temp_schema WHERE name = 'exchange_job_inputs_backup'"
        ),
        Vec::<Vec<Value>>::new()
    );
    connection
        .execute_batch("DROP TRIGGER reject_restored_input;")
        .unwrap();
    migrate(&mut connection).unwrap();
}

#[test]
fn claimed_observations_require_inert_mirrors_and_live_authority_credentials() {
    let mut connection = fixture();
    migrate(&mut connection).unwrap();
    let production = load_production(&connection).unwrap();
    let store =
        SqliteProduction::from_parts(std::path::PathBuf::new(), connection, production).unwrap();
    let id = JobId::from_bytes([2; 16]);
    assert!(matches!(
        store.job(id).unwrap().state(),
        JobState::Claimed(_)
    ));
    store
        .connection
        .execute_batch("UPDATE jobs SET claim_id = NULL, claim_inert = 1 WHERE state = 2;")
        .unwrap();
    assert!(store.job(id).is_err());
    store
        .connection
        .execute_batch("UPDATE exchange_history SET role = 2;")
        .unwrap();
    let JobState::Claimed(claim) = store.job(id).unwrap().state().clone() else {
        panic!("lost observed claim")
    };
    assert_eq!(claim.tool().name(), "Exact tool");
    assert_eq!(claim.expires_at().as_unix_micros(), 9_007_199_254_740_993);
    store
        .connection
        .execute(
            "UPDATE jobs SET claim_id = ?1, claim_inert = 0 WHERE state = 2",
            [[2_u8; 16].as_slice()],
        )
        .unwrap();
    assert!(store.job(id).is_err());
}
