-- Protocol envelopes may exceed SQLite's per-value limit. Fragment their exact
-- canonical bytes internally without changing public chunk identities/digests.
ALTER TABLE exchange_record_chunks RENAME TO exchange_record_chunks_previous;

CREATE TABLE exchange_record_chunks (
    revision_id BLOB NOT NULL REFERENCES revisions(id),
    position INTEGER NOT NULL CHECK (position >= 0),
    fragment_position INTEGER NOT NULL CHECK (fragment_position >= 0),
    document BLOB NOT NULL CHECK (length(document) BETWEEN 1 AND 1048576),
    PRIMARY KEY (revision_id, position, fragment_position)
);

INSERT INTO exchange_record_chunks
SELECT revision_id, position, 0, substr(document, 1, 1048576)
FROM exchange_record_chunks_previous;

INSERT INTO exchange_record_chunks
SELECT revision_id, position, 1, substr(document, 1048577)
FROM exchange_record_chunks_previous WHERE length(document) > 1048576;

DROP TABLE exchange_record_chunks_previous;
UPDATE productions SET schema_version = 24;
