CREATE TABLE job_clock (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    high_water_micros INTEGER
);
INSERT INTO job_clock (singleton, high_water_micros) VALUES (1, NULL);

-- Expire legacy caller-timed claims; their attribution remains observable.
UPDATE jobs SET claim_expires_at_micros = 0 WHERE state = 2;
UPDATE productions SET schema_version = 19;
