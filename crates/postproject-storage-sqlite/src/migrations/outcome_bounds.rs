//! Expanding recovery storage preserves identities, bytes and private bindings.

use rusqlite::{Connection, params};

use super::{MIGRATIONS, apply_migration, migrate, schema_version};

#[test]
fn schema_twenty_five_outcomes_keep_exact_bytes_and_bindings_with_larger_bounds() {
    let mut connection = Connection::open_in_memory().unwrap();
    connection
        .pragma_update(None, "foreign_keys", true)
        .unwrap();
    for migration in &MIGRATIONS[..25] {
        apply_migration(&mut connection, migration).unwrap();
    }
    let bytes = b"retained development outcome";
    let binding = [7_u8; 32];
    connection.execute(
        "INSERT INTO exchange_outcomes VALUES (zeroblob(16), zeroblob(16), zeroblob(16), zeroblob(32), ?1, ?2)",
        params![binding.as_slice(), bytes.as_slice()],
    ).unwrap();
    migrate(&mut connection).unwrap();
    let retained: (Vec<u8>, Vec<u8>) = connection
        .query_row(
            "SELECT capability_binding, outcome FROM exchange_outcomes",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(retained, (binding.to_vec(), bytes.to_vec()));
    for length in [131_073, 524_288] {
        connection
            .execute(
                "UPDATE exchange_outcomes SET outcome = zeroblob(?1)",
                [length],
            )
            .unwrap();
    }
    for length in [0, 524_289] {
        assert!(
            connection
                .execute(
                    "UPDATE exchange_outcomes SET outcome = zeroblob(?1)",
                    [length]
                )
                .is_err()
        );
    }
    assert!(
        connection
            .execute(
                "UPDATE exchange_outcomes SET capability_binding = zeroblob(31)",
                []
            )
            .is_err()
    );
    assert_eq!(schema_version(&connection).unwrap(), 26);
}

#[test]
fn late_failure_rolls_back_outcome_parent_replacement_and_migration_history() {
    let mut connection = Connection::open_in_memory().unwrap();
    for migration in &MIGRATIONS[..25] {
        apply_migration(&mut connection, migration).unwrap();
    }
    connection.execute_batch("INSERT INTO productions VALUES (1, zeroblob(16), 25, 0, NULL);
        INSERT INTO exchange_outcomes VALUES (zeroblob(16), zeroblob(16), zeroblob(16), zeroblob(32), NULL, X'01');
        CREATE TRIGGER reject_version BEFORE UPDATE OF schema_version ON productions
        BEGIN SELECT RAISE(ABORT, 'late migration failure'); END;").unwrap();
    assert!(migrate(&mut connection).is_err());
    assert_eq!(schema_version(&connection).unwrap(), 25);
    let bytes: Vec<u8> = connection
        .query_row("SELECT outcome FROM exchange_outcomes", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(bytes, [1]);
    // The original CHECK is also restored after the failed parent replacement.
    assert!(
        connection
            .execute(
                "UPDATE exchange_outcomes SET outcome = zeroblob(131073)",
                []
            )
            .is_err()
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT count(*) FROM schema_migrations WHERE version = 26",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    connection
        .execute_batch("DROP TRIGGER reject_version")
        .unwrap();
    migrate(&mut connection).unwrap();
    assert_eq!(schema_version(&connection).unwrap(), 26);
}
