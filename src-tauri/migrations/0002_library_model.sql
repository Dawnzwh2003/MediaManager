-- Phase 2 stores facts and constraints only. No scanner or Event Service.
CREATE TABLE db_sequence (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    value INTEGER NOT NULL CHECK (value >= 0)
) STRICT;
INSERT INTO db_sequence VALUES (1, 0);

CREATE TABLE media (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(id)) > 0),
    fingerprint TEXT,
    file_path TEXT NOT NULL CHECK (length(trim(file_path)) > 0),
    file_name TEXT NOT NULL CHECK (length(trim(file_name)) > 0),
    media_type TEXT NOT NULL CHECK (media_type IN ('image', 'video')),
    file_size INTEGER NOT NULL CHECK (file_size >= 0),
    created_time INTEGER,
    modified_time INTEGER NOT NULL,
    imported_at INTEGER NOT NULL,
    last_seen_at INTEGER NOT NULL,
    width INTEGER CHECK (width > 0),
    height INTEGER CHECK (height > 0),
    duration REAL CHECK (duration >= 0),
    thumbnail_path TEXT,
    like_count INTEGER NOT NULL DEFAULT 0 CHECK (like_count >= 0),
    view_count INTEGER NOT NULL DEFAULT 0 CHECK (view_count >= 0),
    play_count INTEGER NOT NULL DEFAULT 0 CHECK (play_count >= 0),
    last_viewed_at INTEGER,
    last_played_at INTEGER,
    playback_position REAL NOT NULL DEFAULT 0 CHECK (playback_position >= 0),
    missing INTEGER NOT NULL DEFAULT 0 CHECK (missing IN (0, 1)),
    state TEXT NOT NULL DEFAULT 'active' CHECK (state IN ('active', 'missing', 'superseded')),
    current_content_version_id INTEGER NOT NULL DEFAULT 1 CHECK (current_content_version_id > 0),
    created_seq INTEGER NOT NULL UNIQUE CHECK (created_seq > 0),
    CHECK ((state = 'active' AND missing = 0) OR (state != 'active' AND missing = 1)),
    FOREIGN KEY (id, current_content_version_id)
        REFERENCES media_revisions(media_id, content_version_id) DEFERRABLE INITIALLY DEFERRED
) STRICT;
-- Fingerprints deliberately have no unique constraint. Path only reserves a live location.
CREATE INDEX media_fingerprint ON media(fingerprint);
CREATE UNIQUE INDEX media_active_path ON media(file_path) WHERE state = 'active';
CREATE INDEX media_imported ON media(imported_at, id);
CREATE INDEX media_likes ON media(like_count, id);
CREATE INDEX media_views ON media(view_count, id);
CREATE INDEX media_plays ON media(play_count, id);
CREATE INDEX media_missing ON media(missing, id);
-- Reject REPLACE as well as ordinary duplicate INSERT, even when recursive triggers are off.
CREATE TRIGGER media_no_replace BEFORE INSERT ON media
WHEN EXISTS(SELECT 1 FROM media WHERE id = NEW.id OR (state = 'active' AND NEW.state = 'active' AND file_path = NEW.file_path))
BEGIN SELECT RAISE(ABORT, 'UNIQUE media identity or active path'); END;

CREATE TABLE media_revisions (
    media_id TEXT NOT NULL REFERENCES media(id) ON DELETE CASCADE,
    content_version_id INTEGER NOT NULL CHECK (content_version_id > 0),
    fingerprint TEXT,
    file_size INTEGER NOT NULL CHECK (file_size >= 0),
    created_at INTEGER NOT NULL,
    PRIMARY KEY (media_id, content_version_id)
) STRICT;

CREATE TABLE media_locations (
    id INTEGER PRIMARY KEY,
    media_id TEXT NOT NULL REFERENCES media(id) ON DELETE CASCADE,
    file_path TEXT NOT NULL,
    file_name TEXT NOT NULL,
    observed_at INTEGER NOT NULL,
    retired_at INTEGER
) STRICT;
CREATE UNIQUE INDEX media_one_current_location ON media_locations(media_id) WHERE retired_at IS NULL;
CREATE INDEX media_location_history ON media_locations(media_id, id);

CREATE TRIGGER media_initialize AFTER INSERT ON media BEGIN
    INSERT INTO media_revisions(media_id, content_version_id, fingerprint, file_size, created_at)
        VALUES(NEW.id, NEW.current_content_version_id, NEW.fingerprint, NEW.file_size, NEW.imported_at);
    INSERT INTO media_locations(media_id, file_path, file_name, observed_at)
        VALUES(NEW.id, NEW.file_path, NEW.file_name, NEW.last_seen_at);
END;
CREATE TRIGGER media_identity_immutable BEFORE UPDATE OF id, imported_at, created_seq ON media
WHEN NEW.id IS NOT OLD.id OR NEW.imported_at IS NOT OLD.imported_at OR NEW.created_seq IS NOT OLD.created_seq
BEGIN SELECT RAISE(ABORT, 'media identity/import history is immutable'); END;
CREATE TRIGGER media_location_changed AFTER UPDATE OF file_path, file_name ON media
WHEN NEW.file_path IS NOT OLD.file_path OR NEW.file_name IS NOT OLD.file_name
BEGIN
    UPDATE media_locations SET retired_at = NEW.last_seen_at WHERE media_id = NEW.id AND retired_at IS NULL;
    INSERT INTO media_locations(media_id, file_path, file_name, observed_at)
        VALUES(NEW.id, NEW.file_path, NEW.file_name, NEW.last_seen_at);
END;
CREATE TRIGGER media_location_superseded AFTER UPDATE OF state ON media
WHEN NEW.state = 'superseded' AND OLD.state != 'superseded'
BEGIN
    UPDATE media_locations SET retired_at = NEW.last_seen_at WHERE media_id = NEW.id AND retired_at IS NULL;
END;

CREATE TABLE system_tags (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(id)) > 0),
    parent_id TEXT,
    source_root TEXT NOT NULL CHECK (length(trim(source_root)) > 0),
    directory_path TEXT NOT NULL UNIQUE CHECK (length(trim(directory_path)) > 0),
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    source TEXT NOT NULL DEFAULT 'filesystem' CHECK (source = 'filesystem'),
    last_seen_at INTEGER NOT NULL,
    CHECK (parent_id IS NULL OR parent_id != id),
    UNIQUE (id, source_root),
    FOREIGN KEY (parent_id, source_root) REFERENCES system_tags(id, source_root) ON DELETE RESTRICT
) STRICT;
CREATE UNIQUE INDEX system_one_root ON system_tags(source_root) WHERE parent_id IS NULL;
CREATE INDEX system_tag_parent ON system_tags(parent_id, id);
CREATE TRIGGER system_tags_no_cycle BEFORE UPDATE OF parent_id ON system_tags
WHEN NEW.parent_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'system tag cycle') WHERE EXISTS (
        WITH RECURSIVE ancestors(id) AS (
            SELECT NEW.parent_id
            UNION
            SELECT tags.parent_id FROM system_tags tags JOIN ancestors ON tags.id = ancestors.id
            WHERE tags.parent_id IS NOT NULL
        ) SELECT 1 FROM ancestors WHERE id = NEW.id
    );
END;

CREATE TABLE custom_tags (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(id)) > 0),
    name TEXT NOT NULL COLLATE NOCASE UNIQUE CHECK (length(trim(name)) > 0 AND name = trim(name)),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;
CREATE TABLE media_system_tags (
    media_id TEXT NOT NULL REFERENCES media(id) ON DELETE CASCADE,
    tag_id TEXT NOT NULL REFERENCES system_tags(id) ON DELETE CASCADE,
    PRIMARY KEY (media_id, tag_id)
) STRICT;
CREATE INDEX system_tag_media ON media_system_tags(tag_id, media_id);
CREATE TABLE media_custom_tags (
    media_id TEXT NOT NULL REFERENCES media(id) ON DELETE CASCADE,
    tag_id TEXT NOT NULL REFERENCES custom_tags(id) ON DELETE CASCADE,
    PRIMARY KEY (media_id, tag_id)
) STRICT;
CREATE INDEX custom_tag_media ON media_custom_tags(tag_id, media_id);

CREATE TABLE like_events (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(id)) > 0),
    ingest_seq INTEGER NOT NULL UNIQUE CHECK (ingest_seq > 0),
    request_id TEXT NOT NULL UNIQUE CHECK (length(trim(request_id)) > 0),
    media_id TEXT NOT NULL REFERENCES media(id) ON DELETE RESTRICT,
    occurred_at INTEGER NOT NULL,
    recorded_at INTEGER NOT NULL,
    policy_version INTEGER NOT NULL DEFAULT 1 CHECK (policy_version > 0)
) STRICT;
CREATE TABLE view_events (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(id)) > 0),
    ingest_seq INTEGER NOT NULL UNIQUE CHECK (ingest_seq > 0),
    request_id TEXT NOT NULL UNIQUE CHECK (length(trim(request_id)) > 0),
    media_id TEXT NOT NULL REFERENCES media(id) ON DELETE RESTRICT,
    activation_id TEXT NOT NULL UNIQUE CHECK (length(trim(activation_id)) > 0),
    occurred_at INTEGER NOT NULL,
    recorded_at INTEGER NOT NULL,
    policy_version INTEGER NOT NULL DEFAULT 1 CHECK (policy_version > 0)
) STRICT;
CREATE TABLE play_events (
    id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(id)) > 0),
    ingest_seq INTEGER NOT NULL UNIQUE CHECK (ingest_seq > 0),
    request_id TEXT NOT NULL UNIQUE CHECK (length(trim(request_id)) > 0),
    media_id TEXT NOT NULL REFERENCES media(id) ON DELETE RESTRICT,
    content_version_id INTEGER NOT NULL,
    player_session_id TEXT NOT NULL UNIQUE CHECK (length(trim(player_session_id)) > 0),
    started_at INTEGER NOT NULL,
    qualified_at INTEGER NOT NULL CHECK (qualified_at >= started_at),
    ended_at INTEGER CHECK (ended_at >= qualified_at),
    start_position REAL NOT NULL CHECK (start_position >= 0),
    end_position REAL NOT NULL CHECK (end_position >= 0),
    watched_seconds REAL NOT NULL CHECK (watched_seconds >= 0),
    completed INTEGER NOT NULL CHECK (completed IN (0, 1)),
    end_reason TEXT,
    policy_version INTEGER NOT NULL DEFAULT 1 CHECK (policy_version > 0),
    FOREIGN KEY (media_id, content_version_id) REFERENCES media_revisions(media_id, content_version_id) ON DELETE RESTRICT
) STRICT;
CREATE INDEX likes_by_media ON like_events(media_id, ingest_seq);
CREATE INDEX views_by_media ON view_events(media_id, ingest_seq);
CREATE INDEX plays_by_media ON play_events(media_id, ingest_seq);

-- These triggers are storage invariants, not qualification or Event Services.
CREATE TRIGGER like_no_replace BEFORE INSERT ON like_events
WHEN EXISTS(SELECT 1 FROM like_events WHERE id = NEW.id OR request_id = NEW.request_id OR ingest_seq = NEW.ingest_seq)
BEGIN SELECT RAISE(ABORT, 'UNIQUE like event identity'); END;
CREATE TRIGGER view_no_replace BEFORE INSERT ON view_events
WHEN EXISTS(SELECT 1 FROM view_events WHERE id = NEW.id OR request_id = NEW.request_id OR ingest_seq = NEW.ingest_seq OR activation_id = NEW.activation_id)
BEGIN SELECT RAISE(ABORT, 'UNIQUE view event identity'); END;
CREATE TRIGGER play_no_replace BEFORE INSERT ON play_events
WHEN EXISTS(SELECT 1 FROM play_events WHERE id = NEW.id OR request_id = NEW.request_id OR ingest_seq = NEW.ingest_seq OR player_session_id = NEW.player_session_id)
BEGIN SELECT RAISE(ABORT, 'UNIQUE play event identity'); END;
CREATE TRIGGER like_counter AFTER INSERT ON like_events BEGIN
    UPDATE media SET like_count = like_count + 1 WHERE id = NEW.media_id;
END;
CREATE TRIGGER view_counter AFTER INSERT ON view_events BEGIN
    UPDATE media SET view_count = view_count + 1,
        last_viewed_at = max(coalesce(last_viewed_at, NEW.occurred_at), NEW.occurred_at) WHERE id = NEW.media_id;
END;
CREATE TRIGGER play_counter AFTER INSERT ON play_events BEGIN
    UPDATE media SET play_count = play_count + 1,
        last_played_at = max(coalesce(last_played_at, NEW.qualified_at), NEW.qualified_at) WHERE id = NEW.media_id;
END;
CREATE TRIGGER like_no_update BEFORE UPDATE ON like_events BEGIN SELECT RAISE(ABORT, 'immutable like event'); END;
CREATE TRIGGER like_no_delete BEFORE DELETE ON like_events BEGIN SELECT RAISE(ABORT, 'immutable like event'); END;
CREATE TRIGGER view_no_update BEFORE UPDATE ON view_events BEGIN SELECT RAISE(ABORT, 'immutable view event'); END;
CREATE TRIGGER view_no_delete BEFORE DELETE ON view_events BEGIN SELECT RAISE(ABORT, 'immutable view event'); END;
CREATE TRIGGER play_no_delete BEFORE DELETE ON play_events BEGIN SELECT RAISE(ABORT, 'immutable play event'); END;
CREATE TRIGGER play_finalize_only BEFORE UPDATE ON play_events
WHEN OLD.ended_at IS NOT NULL OR NEW.ended_at IS NULL
    OR NEW.id IS NOT OLD.id OR NEW.ingest_seq IS NOT OLD.ingest_seq
    OR NEW.request_id IS NOT OLD.request_id OR NEW.media_id IS NOT OLD.media_id
    OR NEW.content_version_id IS NOT OLD.content_version_id OR NEW.player_session_id IS NOT OLD.player_session_id
    OR NEW.started_at IS NOT OLD.started_at OR NEW.qualified_at IS NOT OLD.qualified_at
    OR NEW.start_position IS NOT OLD.start_position OR NEW.policy_version IS NOT OLD.policy_version
    OR NEW.watched_seconds < OLD.watched_seconds
BEGIN SELECT RAISE(ABORT, 'only one play finalization is allowed'); END;

CREATE TABLE playback_states (
    media_id TEXT NOT NULL,
    content_version_id INTEGER NOT NULL,
    position_seconds REAL NOT NULL CHECK (position_seconds >= 0),
    duration_seconds REAL CHECK (duration_seconds >= 0),
    completed INTEGER NOT NULL CHECK (completed IN (0, 1)),
    updated_at INTEGER NOT NULL,
    state_version INTEGER NOT NULL DEFAULT 1 CHECK (state_version > 0),
    PRIMARY KEY (media_id, content_version_id),
    FOREIGN KEY (media_id, content_version_id) REFERENCES media_revisions(media_id, content_version_id) ON DELETE CASCADE
) STRICT;
CREATE TRIGGER playback_identity_immutable BEFORE UPDATE OF media_id, content_version_id ON playback_states
WHEN NEW.media_id IS NOT OLD.media_id OR NEW.content_version_id IS NOT OLD.content_version_id
BEGIN SELECT RAISE(ABORT, 'playback state identity is immutable'); END;
CREATE TRIGGER playback_cache_insert AFTER INSERT ON playback_states BEGIN
    UPDATE media SET playback_position = NEW.position_seconds
        WHERE id = NEW.media_id AND current_content_version_id = NEW.content_version_id;
END;
CREATE TRIGGER playback_cache_update AFTER UPDATE ON playback_states BEGIN
    UPDATE media SET playback_position = NEW.position_seconds
        WHERE id = NEW.media_id AND current_content_version_id = NEW.content_version_id;
END;
CREATE TRIGGER playback_cache_delete AFTER DELETE ON playback_states BEGIN
    UPDATE media SET playback_position = 0
        WHERE id = OLD.media_id AND current_content_version_id = OLD.content_version_id;
END;
CREATE TRIGGER playback_cache_version AFTER UPDATE OF current_content_version_id ON media BEGIN
    UPDATE media SET playback_position = coalesce((SELECT position_seconds FROM playback_states
        WHERE media_id = NEW.id AND content_version_id = NEW.current_content_version_id), 0) WHERE id = NEW.id;
END;

CREATE TABLE quick_browse_sessions (
    id TEXT PRIMARY KEY NOT NULL,
    seed TEXT,
    sort_mode TEXT NOT NULL CHECK (sort_mode IN ('imported', 'random')),
    algorithm_version INTEGER NOT NULL DEFAULT 1 CHECK (algorithm_version > 0),
    filter_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(filter_json)),
    state TEXT NOT NULL DEFAULT 'building' CHECK (state IN ('building', 'ready', 'closed')),
    item_count INTEGER NOT NULL DEFAULT 0 CHECK (item_count >= 0),
    current_ordinal INTEGER CHECK (current_ordinal >= 0),
    created_at INTEGER NOT NULL,
    CHECK (sort_mode != 'random' OR (seed IS NOT NULL AND length(seed) > 0)),
    CHECK (current_ordinal IS NULL OR current_ordinal < item_count)
) STRICT;
CREATE TABLE quick_browse_session_items (
    session_id TEXT NOT NULL REFERENCES quick_browse_sessions(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    media_id TEXT NOT NULL REFERENCES media(id) ON DELETE RESTRICT,
    content_version_id INTEGER NOT NULL,
    PRIMARY KEY (session_id, ordinal),
    UNIQUE (session_id, media_id),
    FOREIGN KEY (media_id, content_version_id) REFERENCES media_revisions(media_id, content_version_id) ON DELETE RESTRICT
) STRICT;
CREATE INDEX quick_browse_media ON quick_browse_session_items(media_id);
CREATE TRIGGER quick_items_insert BEFORE INSERT ON quick_browse_session_items
WHEN (SELECT state FROM quick_browse_sessions WHERE id = NEW.session_id) != 'building'
BEGIN SELECT RAISE(ABORT, 'published session members are immutable'); END;
CREATE TRIGGER quick_items_update BEFORE UPDATE ON quick_browse_session_items
BEGIN SELECT RAISE(ABORT, 'session items are immutable'); END;
CREATE TRIGGER quick_items_delete BEFORE DELETE ON quick_browse_session_items
WHEN EXISTS(SELECT 1 FROM quick_browse_sessions WHERE id = OLD.session_id AND state != 'building')
BEGIN SELECT RAISE(ABORT, 'published session members are immutable'); END;
CREATE TRIGGER quick_session_publish BEFORE UPDATE OF state ON quick_browse_sessions
WHEN NEW.state = 'ready' AND (OLD.state != 'building'
    OR NEW.item_count != (SELECT count(*) FROM quick_browse_session_items WHERE session_id = NEW.id)
    OR (NEW.item_count > 0 AND (SELECT max(ordinal) FROM quick_browse_session_items WHERE session_id = NEW.id) != NEW.item_count - 1))
BEGIN SELECT RAISE(ABORT, 'session must publish contiguous complete members'); END;
CREATE TRIGGER quick_session_frozen BEFORE UPDATE ON quick_browse_sessions
WHEN OLD.state != 'building' AND (NEW.seed IS NOT OLD.seed OR NEW.sort_mode IS NOT OLD.sort_mode
    OR NEW.algorithm_version IS NOT OLD.algorithm_version OR NEW.filter_json IS NOT OLD.filter_json
    OR NEW.item_count IS NOT OLD.item_count OR NEW.state = 'building')
BEGIN SELECT RAISE(ABORT, 'published session ordering is immutable'); END;

CREATE TABLE app_settings (
    key TEXT PRIMARY KEY NOT NULL CHECK (length(trim(key)) > 0),
    value_json TEXT NOT NULL CHECK (json_valid(value_json)),
    updated_at INTEGER NOT NULL
) STRICT;
