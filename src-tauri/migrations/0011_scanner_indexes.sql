-- Scanner lookups include missing instances; the active-path index cannot serve them.
CREATE INDEX media_scan_path ON media(file_path,missing) WHERE state!='superseded';
-- Seek through a root by stable media ID instead of sorting its whole generation.
CREATE INDEX scan_unseen ON scan_evidence(root,media_id,generation);
