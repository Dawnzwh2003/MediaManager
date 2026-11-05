CREATE TABLE app_metadata (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
) STRICT;

INSERT INTO app_metadata (key, value)
VALUES ('database_created_at', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
