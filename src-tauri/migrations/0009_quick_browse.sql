ALTER TABLE quick_browse_sessions ADD COLUMN started_at INTEGER;
ALTER TABLE quick_browse_sessions ADD COLUMN ended_at INTEGER;
ALTER TABLE quick_browse_sessions ADD COLUMN query_sort TEXT NOT NULL DEFAULT 'recent';
ALTER TABLE quick_browse_sessions ADD COLUMN autoplay INTEGER NOT NULL DEFAULT 0 CHECK(autoplay IN(0,1));
ALTER TABLE quick_browse_sessions ADD COLUMN muted INTEGER NOT NULL DEFAULT 1 CHECK(muted IN(0,1));
ALTER TABLE quick_browse_sessions ADD COLUMN build_phase TEXT NOT NULL DEFAULT 'complete';
ALTER TABLE quick_browse_sessions ADD COLUMN build_count INTEGER NOT NULL DEFAULT 0 CHECK(build_count>=0);
ALTER TABLE quick_browse_sessions ADD COLUMN published_count INTEGER NOT NULL DEFAULT 0 CHECK(published_count>=0);
ALTER TABLE quick_browse_sessions ADD COLUMN cancel_requested INTEGER NOT NULL DEFAULT 0 CHECK(cancel_requested IN(0,1));
ALTER TABLE quick_browse_sessions ADD COLUMN snapshot_generation INTEGER;
ALTER TABLE quick_browse_sessions ADD COLUMN error TEXT;
ALTER TABLE quick_browse_sessions ADD COLUMN parent_session_id TEXT REFERENCES quick_browse_sessions(id) ON DELETE RESTRICT;
ALTER TABLE quick_browse_sessions ADD COLUMN request_json TEXT NOT NULL DEFAULT '{}';
UPDATE quick_browse_sessions SET started_at=created_at,
    query_sort=CASE WHEN sort_mode='random' THEN 'random' ELSE 'recent' END,
    build_count=item_count,published_count=item_count;
CREATE INDEX quick_browse_pending ON quick_browse_sessions(state,build_phase,created_at,id);
CREATE TRIGGER quick_browse_configuration_frozen BEFORE UPDATE ON quick_browse_sessions
WHEN NEW.started_at IS NOT OLD.started_at OR NEW.query_sort IS NOT OLD.query_sort
 OR NEW.parent_session_id IS NOT OLD.parent_session_id
 OR NEW.request_json IS NOT OLD.request_json
BEGIN SELECT RAISE(ABORT,'browse configuration is immutable'); END;
CREATE TRIGGER quick_browse_end_once BEFORE UPDATE OF ended_at ON quick_browse_sessions
WHEN (OLD.ended_at IS NOT NULL AND NEW.ended_at IS NOT OLD.ended_at)
 OR NEW.ended_at<NEW.started_at
BEGIN SELECT RAISE(ABORT,'browse end time is immutable'); END;
CREATE TABLE quick_tag_operations (
    request_id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES quick_browse_sessions(id) ON DELETE RESTRICT,
    ordinal INTEGER NOT NULL CHECK(ordinal>=0),
    digit INTEGER NOT NULL CHECK(digit BETWEEN 1 AND 9),
    media_id TEXT NOT NULL REFERENCES media(id) ON DELETE RESTRICT,
    tag_id TEXT NOT NULL,
    tag_name TEXT NOT NULL,
    attached INTEGER NOT NULL CHECK(attached IN(0,1)),
    recorded_at INTEGER NOT NULL
) STRICT;
