-- An image sequence's file names belong to each of its locators, not to its
-- descriptor (ADR 0038). Locators of one resource may now share a URI when
-- their namings differ, so the locator table is rebuilt without its
-- (resource_id, uri) uniqueness; writers keep (resource, URI, naming) unique.

-- Triggers on other tables that read locators would name a missing table
-- while it is replaced, so they are recreated unchanged afterwards.
DROP TRIGGER unresolved_memberships_after_membership_insert;
DROP TRIGGER unresolved_memberships_after_membership_update;
DROP TRIGGER media_root_representations_after_membership_insert;
DROP TRIGGER media_root_representations_after_membership_delete;
DROP TRIGGER media_root_representations_after_membership_update;

CREATE TABLE locators_v15 (
    id BLOB PRIMARY KEY CHECK (length(id) = 16),
    resource_id BLOB NOT NULL REFERENCES resources(id) ON DELETE CASCADE,
    uri TEXT NOT NULL CHECK (length(uri) > 0),
    last_seen_micros INTEGER,
    availability INTEGER NOT NULL CHECK (availability BETWEEN 0 AND 2),
    media_root_name TEXT
        REFERENCES media_roots(name) ON UPDATE CASCADE ON DELETE SET NULL
);

INSERT INTO locators_v15 (
    id, resource_id, uri, last_seen_micros, availability, media_root_name
)
SELECT id, resource_id, uri, last_seen_micros, availability, media_root_name
FROM locators;

-- Dropping the old table deletes its rows without firing triggers, so the
-- derived rows that reference locators are removed by their foreign keys and
-- rebuilt below.
DROP TABLE locators;
ALTER TABLE locators_v15 RENAME TO locators;

CREATE INDEX locators_by_resource ON locators(resource_id, id);
CREATE INDEX locators_by_resource_uri ON locators(resource_id, uri);
CREATE INDEX locators_by_resource_availability
    ON locators(resource_id, availability, id);

INSERT OR IGNORE INTO media_root_representations (
    media_root_name, representation_id, locator_id
)
SELECT l.media_root_name, rr.representation_id, l.id
FROM locators l
JOIN representation_resources rr ON rr.resource_id = l.resource_id
WHERE l.media_root_name IS NOT NULL;

CREATE TRIGGER unresolved_memberships_after_locator_insert
AFTER INSERT ON locators
BEGIN
    DELETE FROM unresolved_memberships WHERE resource_id = NEW.resource_id;
END;

CREATE TRIGGER unresolved_memberships_after_locator_update
AFTER UPDATE OF resource_id ON locators
BEGIN
    DELETE FROM unresolved_memberships WHERE resource_id = NEW.resource_id;
    INSERT OR IGNORE INTO unresolved_memberships (representation_id, resource_id)
    SELECT rr.representation_id, rr.resource_id
    FROM representation_resources rr
    WHERE rr.resource_id = OLD.resource_id AND rr.required = 1
      AND NOT EXISTS (SELECT 1 FROM locators WHERE resource_id = OLD.resource_id);
END;

CREATE TRIGGER unresolved_memberships_after_locator_delete
AFTER DELETE ON locators
WHEN NOT EXISTS (SELECT 1 FROM locators WHERE resource_id = OLD.resource_id)
BEGIN
    INSERT OR IGNORE INTO unresolved_memberships (representation_id, resource_id)
    SELECT rr.representation_id, rr.resource_id
    FROM representation_resources rr
    WHERE rr.resource_id = OLD.resource_id AND rr.required = 1;
END;

CREATE TRIGGER media_root_representations_after_locator_insert
AFTER INSERT ON locators
WHEN NEW.media_root_name IS NOT NULL
BEGIN
    INSERT OR IGNORE INTO media_root_representations (
        media_root_name, representation_id, locator_id
    )
    SELECT NEW.media_root_name, rr.representation_id, NEW.id
    FROM representation_resources rr WHERE rr.resource_id = NEW.resource_id;
END;

CREATE TRIGGER media_root_representations_after_locator_update
AFTER UPDATE OF resource_id, media_root_name ON locators
BEGIN
    DELETE FROM media_root_representations WHERE locator_id = OLD.id;
    INSERT OR IGNORE INTO media_root_representations (
        media_root_name, representation_id, locator_id
    )
    SELECT NEW.media_root_name, rr.representation_id, NEW.id
    FROM representation_resources rr
    WHERE rr.resource_id = NEW.resource_id AND NEW.media_root_name IS NOT NULL;
END;

CREATE TRIGGER unresolved_memberships_after_membership_insert
AFTER INSERT ON representation_resources
WHEN NEW.required = 1
 AND NOT EXISTS (SELECT 1 FROM locators WHERE resource_id = NEW.resource_id)
BEGIN
    INSERT OR IGNORE INTO unresolved_memberships (representation_id, resource_id)
    VALUES (NEW.representation_id, NEW.resource_id);
END;

CREATE TRIGGER unresolved_memberships_after_membership_update
AFTER UPDATE OF representation_id, resource_id, required ON representation_resources
BEGIN
    DELETE FROM unresolved_memberships
    WHERE representation_id = OLD.representation_id AND resource_id = OLD.resource_id;
    INSERT OR IGNORE INTO unresolved_memberships (representation_id, resource_id)
    SELECT NEW.representation_id, NEW.resource_id
    WHERE NEW.required = 1
      AND NOT EXISTS (SELECT 1 FROM locators WHERE resource_id = NEW.resource_id);
END;

CREATE TRIGGER media_root_representations_after_membership_insert
AFTER INSERT ON representation_resources
BEGIN
    INSERT OR IGNORE INTO media_root_representations (
        media_root_name, representation_id, locator_id
    )
    SELECT l.media_root_name, NEW.representation_id, l.id
    FROM locators l
    WHERE l.resource_id = NEW.resource_id AND l.media_root_name IS NOT NULL;
END;

CREATE TRIGGER media_root_representations_after_membership_delete
AFTER DELETE ON representation_resources
BEGIN
    DELETE FROM media_root_representations
    WHERE representation_id = OLD.representation_id
      AND locator_id IN (SELECT id FROM locators WHERE resource_id = OLD.resource_id);
END;

CREATE TRIGGER media_root_representations_after_membership_update
AFTER UPDATE OF representation_id, resource_id ON representation_resources
BEGIN
    DELETE FROM media_root_representations
    WHERE representation_id = OLD.representation_id
      AND locator_id IN (SELECT id FROM locators WHERE resource_id = OLD.resource_id);
    INSERT OR IGNORE INTO media_root_representations (
        media_root_name, representation_id, locator_id
    )
    SELECT l.media_root_name, NEW.representation_id, l.id
    FROM locators l
    WHERE l.resource_id = NEW.resource_id AND l.media_root_name IS NOT NULL;
END;

CREATE TABLE locator_sequence_namings (
    locator_id BLOB PRIMARY KEY REFERENCES locators(id) ON DELETE CASCADE,
    prefix TEXT NOT NULL COLLATE BINARY,
    suffix TEXT NOT NULL COLLATE BINARY,
    padding INTEGER NOT NULL CHECK (padding BETWEEN 0 AND 32),
    CHECK (length(CAST(prefix AS BLOB)) + length(CAST(suffix AS BLOB)) BETWEEN 1 AND 1024)
) WITHOUT ROWID;

INSERT INTO locator_sequence_namings (locator_id, prefix, suffix, padding)
SELECT l.id, s.prefix, s.suffix, s.padding
FROM locators l
JOIN image_sequences s ON s.resource_id = l.resource_id;

-- Rebuild the descriptor tables without the names. The missing-frame table
-- references the descriptor table, so both are copied before either old
-- table is dropped, child first.
CREATE TABLE image_sequences_v15 (
    representation_id BLOB PRIMARY KEY
        REFERENCES representations(id) ON DELETE CASCADE,
    resource_id BLOB NOT NULL UNIQUE REFERENCES resources(id) ON DELETE RESTRICT,
    start_frame INTEGER NOT NULL,
    end_frame INTEGER NOT NULL,
    frame_step INTEGER NOT NULL CHECK (frame_step BETWEEN 1 AND 4294967295),
    rate_numerator INTEGER NOT NULL CHECK (rate_numerator BETWEEN 1 AND 4294967295),
    rate_denominator INTEGER NOT NULL CHECK (rate_denominator BETWEEN 1 AND 4294967295),
    CHECK (end_frame >= start_frame)
);

INSERT INTO image_sequences_v15 (
    representation_id, resource_id, start_frame, end_frame, frame_step,
    rate_numerator, rate_denominator
)
SELECT representation_id, resource_id, start_frame, end_frame, frame_step,
       rate_numerator, rate_denominator
FROM image_sequences;

CREATE TABLE image_sequence_missing_frames_v15 (
    representation_id BLOB NOT NULL
        REFERENCES image_sequences_v15(representation_id) ON DELETE CASCADE,
    frame INTEGER NOT NULL,
    PRIMARY KEY (representation_id, frame)
);

INSERT INTO image_sequence_missing_frames_v15 (representation_id, frame)
SELECT representation_id, frame FROM image_sequence_missing_frames;

DROP TABLE image_sequence_missing_frames;
DROP TABLE image_sequences;
ALTER TABLE image_sequences_v15 RENAME TO image_sequences;
ALTER TABLE image_sequence_missing_frames_v15 RENAME TO image_sequence_missing_frames;

UPDATE productions SET schema_version = 15;
