-- Preserve authority-private recovery identities, bindings and original bytes.
CREATE TABLE exchange_outcomes_expanded (
    history_id BLOB NOT NULL CHECK (length(history_id) = 16),
    client_id BLOB NOT NULL CHECK (length(client_id) = 16),
    request_id BLOB NOT NULL CHECK (length(request_id) = 16),
    request_digest BLOB NOT NULL CHECK (length(request_digest) = 32),
    capability_binding BLOB CHECK (capability_binding IS NULL OR length(capability_binding) = 32),
    outcome BLOB NOT NULL CHECK (length(outcome) BETWEEN 1 AND 524288),
    PRIMARY KEY (history_id, client_id, request_id)
);

INSERT INTO exchange_outcomes_expanded
    (history_id, client_id, request_id, request_digest, capability_binding, outcome)
SELECT history_id, client_id, request_id, request_digest, capability_binding, outcome
FROM exchange_outcomes;

DROP TABLE exchange_outcomes;
ALTER TABLE exchange_outcomes_expanded RENAME TO exchange_outcomes;
UPDATE productions SET schema_version = 26;
