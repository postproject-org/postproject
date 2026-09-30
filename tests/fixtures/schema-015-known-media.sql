INSERT INTO productions (
    singleton, id, schema_version, created_at_micros, display_name
) VALUES (1, x'10101010101040109010101010101010', 15, 0, 'Schema 15 lookup fixture');

INSERT INTO assets (id, created_at_micros, display_name, import_source)
VALUES (x'2020202020204020A020202020202020', 1, 'Known clip', 'fixture');

INSERT INTO representations (id, asset_id, kind, structure_kind)
VALUES (
    x'3030303030304030B030303030303030',
    x'2020202020204020A020202020202020',
    0,
    0
);

INSERT INTO resources (id, file_size_bytes, modified_at_micros)
VALUES (x'40404040404040408040404040404040', 12, 1);

INSERT INTO representation_resources (
    representation_id, resource_id, position, role, required
) VALUES (
    x'3030303030304030B030303030303030',
    x'40404040404040408040404040404040',
    0,
    NULL,
    1
);

INSERT INTO resource_fingerprints (
    resource_id, algorithm, algorithm_version, value
) VALUES (
    x'40404040404040408040404040404040',
    'fixture-host',
    9,
    x'666978747572652D66696E6765727072696E74'
);

INSERT INTO locators (
    id, resource_id, uri, last_seen_micros, availability, media_root_name
) VALUES (
    x'50505050505040509050505050505050',
    x'40404040404040408040404040404040',
    'file:///schema-15/known.mov',
    1,
    1,
    NULL
);
