-- Scanner state is separate from immutable media/event history.
CREATE TABLE scan_roots (
    path TEXT PRIMARY KEY NOT NULL,
    generation INTEGER NOT NULL DEFAULT 0,
    state TEXT NOT NULL CHECK(state IN ('running','complete','interrupted','unavailable'))
) STRICT;
CREATE TABLE scan_evidence (
    media_id TEXT PRIMARY KEY REFERENCES media(id) ON DELETE RESTRICT,
    root TEXT NOT NULL REFERENCES scan_roots(path),
    file_key TEXT,
    stat TEXT NOT NULL,
    generation INTEGER NOT NULL,
    reason TEXT NOT NULL
) STRICT;
CREATE INDEX scan_identity ON scan_evidence(root,file_key);
CREATE INDEX scan_generation ON scan_evidence(root,generation,media_id);
CREATE TABLE scan_queue (
    root TEXT NOT NULL REFERENCES scan_roots(path),
    path TEXT NOT NULL,
    kind TEXT NOT NULL,
    done INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY(root,path)
) STRICT;
CREATE INDEX scan_queue_pending ON scan_queue(root,kind,done,path);
CREATE TABLE scan_observations (
    root TEXT NOT NULL REFERENCES scan_roots(path),
    path TEXT NOT NULL,
    file_key TEXT,
    stat TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    size INTEGER NOT NULL,
    modified INTEGER NOT NULL,
    created INTEGER,
    kind TEXT NOT NULL,
    PRIMARY KEY(root,path)
) STRICT;
CREATE INDEX scan_observed_identity ON scan_observations(root,file_key);
