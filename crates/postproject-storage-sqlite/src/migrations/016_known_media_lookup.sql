CREATE INDEX locators_by_uri_resource
    ON locators(uri, resource_id);

CREATE INDEX resource_fingerprints_by_value_resource
    ON resource_fingerprints(algorithm, algorithm_version, value, resource_id);

UPDATE productions SET schema_version = 16;
