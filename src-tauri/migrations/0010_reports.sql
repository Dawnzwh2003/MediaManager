-- Read models only: no historical events or media identities are rewritten.
CREATE INDEX history_likes_time ON like_events(occurred_at,media_id);
CREATE INDEX history_views_time ON view_events(occurred_at,media_id);
CREATE INDEX history_plays_time ON play_events(qualified_at,media_id);
CREATE INDEX continue_watching_updated ON playback_states(updated_at DESC,media_id DESC)
 WHERE position_seconds>0;

CREATE TABLE report_generation(id INTEGER PRIMARY KEY CHECK(id=1),value INTEGER NOT NULL) STRICT;
INSERT INTO report_generation VALUES(1,0);
CREATE TRIGGER report_progress_insert AFTER INSERT ON playback_states BEGIN
 UPDATE report_generation SET value=value+1;
END;
CREATE TRIGGER report_progress_update AFTER UPDATE ON playback_states BEGIN
 UPDATE report_generation SET value=value+1;
END;
CREATE TRIGGER report_progress_delete AFTER DELETE ON playback_states BEGIN
 UPDATE report_generation SET value=value+1;
END;
CREATE TRIGGER report_content_update AFTER UPDATE OF current_content_version_id,duration,imported_at ON media
WHEN NEW.current_content_version_id IS NOT OLD.current_content_version_id
 OR NEW.duration IS NOT OLD.duration OR NEW.imported_at IS NOT OLD.imported_at
BEGIN UPDATE report_generation SET value=value+1; END;

-- The current recovery state defines completion, independently of any Play event.
-- Older clients left playback_states.completed unchanged; derive it from facts
-- rather than changing immutable events or backfilling an unbounded write batch.
CREATE VIEW current_playback_progress AS
 SELECT s.media_id,s.updated_at,s.position_seconds,
 CASE WHEN s.duration_seconds>0 THEN s.duration_seconds
      WHEN m.duration>0 THEN m.duration ELSE NULL END AS duration_seconds,
 CASE WHEN coalesce(nullif(s.duration_seconds,0),m.duration)>0 THEN
   s.position_seconds/coalesce(nullif(s.duration_seconds,0),m.duration)>=0.95
   OR coalesce(nullif(s.duration_seconds,0),m.duration)-s.position_seconds
      <=min(10.0,coalesce(nullif(s.duration_seconds,0),m.duration)*0.05)
 ELSE 0 END AS completed
 FROM playback_states s INDEXED BY continue_watching_updated CROSS JOIN media m ON m.id=s.media_id
 AND m.current_content_version_id=s.content_version_id
 WHERE s.position_seconds>0 AND m.state='active' AND m.media_type='video';
