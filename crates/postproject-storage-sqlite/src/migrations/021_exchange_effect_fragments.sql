-- Internal fragments retain authored effects inside the native commit. These
-- are not a complete public record until every mutation family is captured.
CREATE TABLE exchange_effect_fragments (
    revision_id BLOB NOT NULL REFERENCES revisions(id),
    effect_position INTEGER NOT NULL CHECK (effect_position >= 0),
    fragment_position INTEGER NOT NULL CHECK (fragment_position >= 0),
    payload BLOB NOT NULL CHECK (length(payload) BETWEEN 1 AND 1048576),
    PRIMARY KEY (revision_id, effect_position, fragment_position)
);

UPDATE productions SET schema_version = 21;
