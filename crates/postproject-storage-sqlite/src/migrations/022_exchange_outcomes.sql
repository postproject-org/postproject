-- Outcomes are authority-private recovery state, not portable checkpoints.
CREATE TABLE exchange_outcomes (
    history_id BLOB NOT NULL CHECK (length(history_id) = 16),
    client_id BLOB NOT NULL CHECK (length(client_id) = 16),
    request_id BLOB NOT NULL CHECK (length(request_id) = 16),
    request_digest BLOB NOT NULL CHECK (length(request_digest) = 32),
    capability_binding BLOB CHECK (capability_binding IS NULL OR length(capability_binding) = 32),
    outcome BLOB NOT NULL CHECK (length(outcome) BETWEEN 1 AND 131072),
    PRIMARY KEY (history_id, client_id, request_id)
);

UPDATE productions SET schema_version = 22;
