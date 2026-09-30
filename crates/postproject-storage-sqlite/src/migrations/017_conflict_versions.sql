CREATE TABLE conflict_versions (
    conflict_key BLOB PRIMARY KEY,
    last_changed_revision_id BLOB NOT NULL
        REFERENCES revisions(id) ON DELETE RESTRICT,
    last_changed_revision_sequence INTEGER NOT NULL
        CHECK (last_changed_revision_sequence > 0)
);

CREATE TABLE conflict_migration_baseline (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    revision_id BLOB NOT NULL REFERENCES revisions(id) ON DELETE RESTRICT,
    revision_sequence INTEGER NOT NULL CHECK (revision_sequence > 0)
);

INSERT INTO conflict_migration_baseline (singleton, revision_id, revision_sequence)
SELECT 1, id, sequence FROM revisions ORDER BY sequence DESC LIMIT 1;

UPDATE productions SET schema_version = 17;
