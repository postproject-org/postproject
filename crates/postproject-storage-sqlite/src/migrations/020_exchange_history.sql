-- A generation is allocated once; opening and checkpoint export never replace it.
CREATE TABLE exchange_history (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    history_id BLOB NOT NULL CHECK (length(history_id) = 16),
    instance_id BLOB NOT NULL CHECK (length(instance_id) = 16),
    role INTEGER NOT NULL CHECK (role IN (1, 2)),
    floor_revision_id BLOB REFERENCES revisions(id),
    floor_sequence INTEGER NOT NULL CHECK (floor_sequence >= 0),
    anchor_digest BLOB CHECK (anchor_digest IS NULL OR length(anchor_digest) = 32),
    CHECK ((floor_sequence = 0 AND floor_revision_id IS NULL) OR
           (floor_sequence > 0 AND floor_revision_id IS NOT NULL))
);

INSERT INTO exchange_history (
    singleton, history_id, instance_id, role, floor_revision_id, floor_sequence
) VALUES (
    1, randomblob(16), randomblob(16), 1,
    (SELECT id FROM revisions ORDER BY sequence DESC LIMIT 1),
    COALESCE((SELECT MAX(sequence) FROM revisions), 0)
);

UPDATE productions SET schema_version = 20;
