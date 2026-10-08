-- Complete framed record bodies, separate from earlier partial effect capture.
CREATE TABLE exchange_records (
    revision_id BLOB PRIMARY KEY REFERENCES revisions(id),
    sequence INTEGER NOT NULL UNIQUE CHECK (sequence > 0),
    manifest BLOB NOT NULL CHECK (length(manifest) BETWEEN 1 AND 262144)
);

CREATE TABLE exchange_record_chunks (
    revision_id BLOB NOT NULL REFERENCES revisions(id),
    position INTEGER NOT NULL CHECK (position >= 0),
    document BLOB NOT NULL CHECK (length(document) BETWEEN 1 AND 2097152),
    PRIMARY KEY (revision_id, position)
);

-- Partial development captures are not a fabricated complete replay prefix.
UPDATE exchange_history SET
    floor_revision_id = (SELECT id FROM revisions ORDER BY sequence DESC LIMIT 1),
    floor_sequence = COALESCE((SELECT MAX(sequence) FROM revisions), 0),
    anchor_digest = NULL;

UPDATE productions SET schema_version = 23;
