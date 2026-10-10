-- Recovery retains earlier anchors without turning them into replay positions.
CREATE TABLE exchange_prior_anchors (
    floor_sequence INTEGER PRIMARY KEY CHECK (floor_sequence >= 0),
    floor_revision_id BLOB REFERENCES revisions(id),
    anchor_digest BLOB NOT NULL CHECK (length(anchor_digest) = 32),
    CHECK ((floor_sequence = 0 AND floor_revision_id IS NULL) OR
           (floor_sequence > 0 AND floor_revision_id IS NOT NULL))
);

UPDATE productions SET schema_version = 27;
