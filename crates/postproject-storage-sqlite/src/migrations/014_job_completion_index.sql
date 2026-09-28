CREATE INDEX jobs_by_completion_activity
    ON jobs(completion_activity_id)
    WHERE completion_activity_id IS NOT NULL;

UPDATE productions SET schema_version = 14;
