CREATE TABLE jobs_exchange (
    id BLOB PRIMARY KEY CHECK (length(id) = 16),
    kind TEXT NOT NULL COLLATE BINARY
        CHECK (length(CAST(kind AS BLOB)) BETWEEN 1 AND 128),
    output_asset_id BLOB NOT NULL
        REFERENCES assets(id) ON DELETE RESTRICT,
    output_representation_kind INTEGER NOT NULL
        CHECK (output_representation_kind BETWEEN 0 AND 3),
    target_root TEXT COLLATE BINARY CHECK (
        target_root IS NULL OR
        length(CAST(target_root AS BLOB)) BETWEEN 1 AND 128
    ),
    state INTEGER NOT NULL CHECK (state BETWEEN 1 AND 5),
    claim_inert INTEGER NOT NULL DEFAULT 0 CHECK (claim_inert IN (0, 1)),
    claim_id BLOB UNIQUE CHECK (claim_id IS NULL OR length(claim_id) = 16),
    claim_tool_name TEXT COLLATE BINARY CHECK (
        claim_tool_name IS NULL OR
        length(CAST(claim_tool_name AS BLOB)) BETWEEN 1 AND 256
    ),
    claim_tool_version TEXT COLLATE BINARY CHECK (
        claim_tool_version IS NULL OR
        length(CAST(claim_tool_version AS BLOB)) BETWEEN 1 AND 128
    ),
    claim_tool_uri TEXT COLLATE BINARY CHECK (
        claim_tool_uri IS NULL OR
        length(CAST(claim_tool_uri AS BLOB)) BETWEEN 1 AND 4096
    ),
    claim_agent_name TEXT COLLATE BINARY CHECK (
        claim_agent_name IS NULL OR
        length(CAST(claim_agent_name AS BLOB)) BETWEEN 1 AND 256
    ),
    claim_agent_scheme TEXT COLLATE BINARY CHECK (
        claim_agent_scheme IS NULL OR
        length(CAST(claim_agent_scheme AS BLOB)) BETWEEN 1 AND 255
    ),
    claim_agent_value TEXT COLLATE BINARY CHECK (
        claim_agent_value IS NULL OR
        length(CAST(claim_agent_value AS BLOB)) BETWEEN 1 AND 4096
    ),
    claim_agent_qualifier TEXT COLLATE BINARY CHECK (
        claim_agent_qualifier IS NULL OR
        length(CAST(claim_agent_qualifier AS BLOB)) BETWEEN 1 AND 1024
    ),
    claim_expires_at_micros INTEGER,
    completion_activity_id BLOB
        REFERENCES activities(id) ON DELETE RESTRICT,
    completion_representation_id BLOB
        REFERENCES representations(id) ON DELETE RESTRICT,
    failure_diagnostic TEXT COLLATE BINARY CHECK (
        failure_diagnostic IS NULL OR
        length(CAST(failure_diagnostic AS BLOB)) BETWEEN 1 AND 4096
    ),
    CHECK (
        (claim_agent_scheme IS NULL AND claim_agent_value IS NULL AND
         claim_agent_qualifier IS NULL) OR
        (claim_agent_scheme IS NOT NULL AND claim_agent_value IS NOT NULL)
    ),
    CHECK (
        (state IN (1, 5) AND claim_inert = 0 AND claim_id IS NULL AND claim_tool_name IS NULL AND
         claim_tool_version IS NULL AND claim_tool_uri IS NULL AND
         claim_agent_name IS NULL AND claim_agent_scheme IS NULL AND
         claim_agent_value IS NULL AND claim_agent_qualifier IS NULL AND
         claim_expires_at_micros IS NULL AND completion_activity_id IS NULL AND
         completion_representation_id IS NULL AND failure_diagnostic IS NULL) OR
        (state = 2 AND ((claim_inert = 0 AND claim_id IS NOT NULL) OR
                        (claim_inert = 1 AND claim_id IS NULL)) AND
         claim_tool_name IS NOT NULL AND
         claim_expires_at_micros IS NOT NULL AND completion_activity_id IS NULL AND
         completion_representation_id IS NULL AND failure_diagnostic IS NULL) OR
        (state = 3 AND claim_inert = 0 AND claim_id IS NULL AND claim_tool_name IS NULL AND
         claim_tool_version IS NULL AND claim_tool_uri IS NULL AND
         claim_agent_name IS NULL AND claim_agent_scheme IS NULL AND
         claim_agent_value IS NULL AND claim_agent_qualifier IS NULL AND
         claim_expires_at_micros IS NULL AND completion_activity_id IS NOT NULL AND
         completion_representation_id IS NOT NULL AND failure_diagnostic IS NULL) OR
        (state = 4 AND claim_inert = 0 AND claim_id IS NULL AND claim_tool_name IS NULL AND
         claim_tool_version IS NULL AND claim_tool_uri IS NULL AND
         claim_agent_name IS NULL AND claim_agent_scheme IS NULL AND
         claim_agent_value IS NULL AND claim_agent_qualifier IS NULL AND
         claim_expires_at_micros IS NULL AND completion_activity_id IS NULL AND
         completion_representation_id IS NULL AND failure_diagnostic IS NOT NULL)
    )
);

-- Preserve child rows while the enforced cascading parent is replaced.
CREATE TEMP TABLE exchange_job_inputs_backup AS SELECT * FROM job_inputs;
DROP TRIGGER delete_job_attachments;
INSERT INTO jobs_exchange (
    id, kind, output_asset_id,
    output_representation_kind, target_root, state,
    claim_id, claim_tool_name, claim_tool_version,
    claim_tool_uri, claim_agent_name, claim_agent_scheme,
    claim_agent_value, claim_agent_qualifier, claim_expires_at_micros,
    completion_activity_id, completion_representation_id, failure_diagnostic
) SELECT
    id, kind, output_asset_id,
    output_representation_kind, target_root, state,
    claim_id, claim_tool_name, claim_tool_version,
    claim_tool_uri, claim_agent_name, claim_agent_scheme,
    claim_agent_value, claim_agent_qualifier, claim_expires_at_micros,
    completion_activity_id, completion_representation_id, failure_diagnostic
FROM jobs;
DROP TABLE jobs;
ALTER TABLE jobs_exchange RENAME TO jobs;
INSERT INTO job_inputs SELECT * FROM exchange_job_inputs_backup;
DROP TABLE exchange_job_inputs_backup;

CREATE INDEX jobs_by_state_kind ON jobs(state, kind, id);
CREATE INDEX jobs_by_kind_id ON jobs(kind, id);
CREATE INDEX jobs_by_completion_activity ON jobs(completion_activity_id)
    WHERE completion_activity_id IS NOT NULL;
CREATE TRIGGER delete_job_attachments AFTER DELETE ON jobs
BEGIN
    DELETE FROM metadata_assertions WHERE target_kind = 5 AND target_id = OLD.id;
END;

UPDATE productions SET schema_version = 25;
