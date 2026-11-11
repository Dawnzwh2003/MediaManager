-- Virtual keys keep nullable dates and case-folded names seekable in both directions.
ALTER TABLE media ADD COLUMN filename_sort TEXT GENERATED ALWAYS AS (lower(file_name)) VIRTUAL;
ALTER TABLE media ADD COLUMN created_sort INTEGER GENERATED ALWAYS AS (coalesce(created_time, -9223372036854775808)) VIRTUAL;
ALTER TABLE media ADD COLUMN viewed_sort INTEGER GENERATED ALWAYS AS (coalesce(last_viewed_at, -9223372036854775808)) VIRTUAL;
ALTER TABLE media ADD COLUMN played_sort INTEGER GENERATED ALWAYS AS (coalesce(last_played_at, -9223372036854775808)) VIRTUAL;

DROP INDEX media_library_name;
CREATE INDEX media_library_name ON media(state,filename_sort,id);
CREATE INDEX media_library_views ON media(state,view_count,id);
CREATE INDEX media_library_plays ON media(state,play_count,id);
CREATE INDEX media_library_created ON media(state,created_sort,id);
CREATE INDEX media_library_viewed ON media(state,viewed_sort,id);
CREATE INDEX media_library_played ON media(state,played_sort,id);
CREATE INDEX media_library_type_recent ON media(state,media_type,imported_at,id);

CREATE TRIGGER library_query_facts_update
AFTER UPDATE OF view_count,play_count,created_time,last_viewed_at,last_played_at ON media
WHEN NEW.view_count IS NOT OLD.view_count OR NEW.play_count IS NOT OLD.play_count
 OR NEW.created_time IS NOT OLD.created_time OR NEW.last_viewed_at IS NOT OLD.last_viewed_at
 OR NEW.last_played_at IS NOT OLD.last_played_at
BEGIN UPDATE library_generation SET value=value+1; END;
