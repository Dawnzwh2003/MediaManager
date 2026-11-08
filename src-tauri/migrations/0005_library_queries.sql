-- Generation binds live cursors to a consistent ordering/filter snapshot.
CREATE TABLE library_generation (id INTEGER PRIMARY KEY CHECK(id=1), value INTEGER NOT NULL) STRICT;
INSERT INTO library_generation VALUES(1,0);
CREATE INDEX media_library_name ON media(state,file_name,id);
CREATE INDEX media_library_recent ON media(state,imported_at,id);
CREATE INDEX media_library_likes ON media(state,like_count,id);
CREATE TRIGGER library_media_insert AFTER INSERT ON media BEGIN UPDATE library_generation SET value=value+1; END;
CREATE TRIGGER library_media_delete AFTER DELETE ON media BEGIN UPDATE library_generation SET value=value+1; END;
CREATE TRIGGER library_media_update AFTER UPDATE OF file_name,media_type,state,like_count ON media
WHEN NEW.file_name IS NOT OLD.file_name OR NEW.media_type IS NOT OLD.media_type OR NEW.state IS NOT OLD.state OR NEW.like_count IS NOT OLD.like_count
BEGIN UPDATE library_generation SET value=value+1; END;
CREATE TRIGGER library_custom_insert AFTER INSERT ON custom_tags BEGIN UPDATE library_generation SET value=value+1; END;
CREATE TRIGGER library_custom_update AFTER UPDATE ON custom_tags BEGIN UPDATE library_generation SET value=value+1; END;
CREATE TRIGGER library_custom_delete AFTER DELETE ON custom_tags BEGIN UPDATE library_generation SET value=value+1; END;
CREATE TRIGGER library_link_insert AFTER INSERT ON media_custom_tags BEGIN UPDATE library_generation SET value=value+1; END;
CREATE TRIGGER library_link_delete AFTER DELETE ON media_custom_tags BEGIN UPDATE library_generation SET value=value+1; END;
CREATE TRIGGER library_system_link_insert AFTER INSERT ON media_system_tags BEGIN UPDATE library_generation SET value=value+1; END;
CREATE TRIGGER library_system_link_delete AFTER DELETE ON media_system_tags BEGIN UPDATE library_generation SET value=value+1; END;
CREATE TRIGGER library_system_insert AFTER INSERT ON system_tags BEGIN UPDATE library_generation SET value=value+1; END;
CREATE TRIGGER library_system_update AFTER UPDATE OF parent_id,name ON system_tags
WHEN NEW.parent_id IS NOT OLD.parent_id OR NEW.name IS NOT OLD.name
BEGIN UPDATE library_generation SET value=value+1; END;
CREATE TRIGGER library_system_delete AFTER DELETE ON system_tags BEGIN UPDATE library_generation SET value=value+1; END;
