-- Recovery state only. No Play lifecycle or event/counter writes.
CREATE TABLE playback_owners (
    media_id TEXT NOT NULL,
    content_version_id INTEGER NOT NULL,
    owner_id TEXT NOT NULL UNIQUE,
    last_sequence INTEGER NOT NULL DEFAULT 0 CHECK(last_sequence>=0),
    last_position REAL,
    last_duration REAL,
    PRIMARY KEY(media_id,content_version_id),
    FOREIGN KEY(media_id,content_version_id) REFERENCES media_revisions(media_id,content_version_id) ON DELETE CASCADE
) STRICT;
