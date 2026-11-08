-- Disposable work state; source media and event history remain independent.
ALTER TABLE media ADD COLUMN video_codec TEXT;
ALTER TABLE media ADD COLUMN audio_codec TEXT;
ALTER TABLE media ADD COLUMN container TEXT;
ALTER TABLE media ADD COLUMN frame_rate REAL CHECK(frame_rate > 0);
CREATE TABLE media_derivatives (
    media_id TEXT PRIMARY KEY NOT NULL REFERENCES media(id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','running','ready','failed')),
    metadata_status TEXT NOT NULL DEFAULT 'pending' CHECK(metadata_status IN ('pending','ready','failed')),
    metadata_error TEXT,
    thumbnail_error TEXT,
    cache_key TEXT,
    generator_version INTEGER NOT NULL DEFAULT 1,
    attempts INTEGER NOT NULL DEFAULT 0 CHECK(attempts >= 0)
) STRICT;
CREATE INDEX derivatives_pending ON media_derivatives(status,media_id);
INSERT INTO media_derivatives(media_id) SELECT id FROM media WHERE state='active';
-- Previous paths had no verifiable generator/content key; rebuild only disposable thumbnails.
UPDATE media SET thumbnail_path=NULL;
CREATE TRIGGER derivatives_insert AFTER INSERT ON media BEGIN
    INSERT INTO media_derivatives(media_id) VALUES(NEW.id);
END;
-- Keep unavailable rows out of the hot pending index; restored ready caches remain reusable.
CREATE TRIGGER derivatives_suspend AFTER UPDATE OF state ON media
WHEN NEW.state!='active' AND OLD.state='active'
BEGIN
    UPDATE media_derivatives SET status='failed',thumbnail_error='source_unavailable'
        WHERE media_id=NEW.id AND status IN ('pending','running');
END;
CREATE TRIGGER derivatives_restore AFTER UPDATE OF state ON media
WHEN NEW.state='active' AND OLD.state!='active'
BEGIN
    UPDATE media_derivatives SET status='pending',thumbnail_error=NULL
        WHERE media_id=NEW.id AND thumbnail_error='source_unavailable';
END;
-- A location change alone preserves the cache. Content evidence never does.
CREATE TRIGGER derivatives_invalidate AFTER UPDATE OF current_content_version_id,fingerprint,file_size,modified_time ON media
WHEN NEW.current_content_version_id IS NOT OLD.current_content_version_id
  OR NEW.fingerprint IS NOT OLD.fingerprint OR NEW.file_size IS NOT OLD.file_size
  OR NEW.modified_time IS NOT OLD.modified_time
BEGIN
    UPDATE media SET width=NULL,height=NULL,duration=NULL,thumbnail_path=NULL,
        video_codec=NULL,audio_codec=NULL,container=NULL,frame_rate=NULL WHERE id=NEW.id;
    INSERT INTO media_derivatives(media_id) VALUES(NEW.id)
    ON CONFLICT(media_id) DO UPDATE SET status='pending',metadata_status='pending',
        metadata_error=NULL,thumbnail_error=NULL,cache_key=NULL;
END;
