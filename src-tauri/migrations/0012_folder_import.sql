-- Additive: external-source provenance is separate from physical System Tags.
CREATE TABLE storage_roots(id TEXT PRIMARY KEY,path TEXT NOT NULL UNIQUE,created_at INTEGER NOT NULL) STRICT;
CREATE TABLE import_sources(id TEXT PRIMARY KEY,root_path TEXT NOT NULL UNIQUE,label TEXT NOT NULL,created_at INTEGER NOT NULL) STRICT;
CREATE TABLE import_source_tags(
 id TEXT PRIMARY KEY,source_id TEXT NOT NULL REFERENCES import_sources(id) ON DELETE RESTRICT,
 parent_id TEXT,relative_dir TEXT NOT NULL CHECK(length(relative_dir)>0),name TEXT NOT NULL,
 UNIQUE(source_id,relative_dir),UNIQUE(source_id,id),
 FOREIGN KEY(source_id,parent_id) REFERENCES import_source_tags(source_id,id) ON DELETE RESTRICT
) STRICT;
CREATE INDEX import_tag_parent ON import_source_tags(source_id,parent_id,id);
CREATE INDEX import_tag_children ON import_source_tags(parent_id,id);
CREATE TABLE media_import_sources(
 media_id TEXT NOT NULL REFERENCES media(id) ON DELETE RESTRICT,
 source_id TEXT NOT NULL REFERENCES import_sources(id) ON DELETE RESTRICT,
 relative_path TEXT NOT NULL,first_seen_at INTEGER NOT NULL,last_seen_at INTEGER NOT NULL,
 PRIMARY KEY(media_id,source_id,relative_path)
) STRICT;
CREATE INDEX import_source_media ON media_import_sources(source_id,relative_path,media_id);
CREATE TABLE media_import_tags(
 media_id TEXT NOT NULL REFERENCES media(id) ON DELETE RESTRICT,
 tag_id TEXT NOT NULL REFERENCES import_source_tags(id) ON DELETE RESTRICT,
 PRIMARY KEY(media_id,tag_id)
) STRICT;
CREATE INDEX import_tag_media ON media_import_tags(tag_id,media_id);
CREATE TABLE media_full_hashes(
 media_id TEXT NOT NULL,content_version_id INTEGER NOT NULL,algorithm TEXT NOT NULL CHECK(algorithm='sha256-v1'),
 full_hash TEXT NOT NULL CHECK(length(full_hash)=64),file_size INTEGER NOT NULL,stat TEXT NOT NULL,
 PRIMARY KEY(media_id,content_version_id),
 FOREIGN KEY(media_id,content_version_id) REFERENCES media_revisions(media_id,content_version_id) ON DELETE RESTRICT
) STRICT;
CREATE INDEX import_full_hash ON media_full_hashes(full_hash,file_size);
CREATE TABLE managed_media(
 media_id TEXT PRIMARY KEY REFERENCES media(id) ON DELETE RESTRICT,
 storage_root_id TEXT NOT NULL REFERENCES storage_roots(id) ON DELETE RESTRICT,
 relative_path TEXT NOT NULL,UNIQUE(storage_root_id,relative_path)
) STRICT;
CREATE INDEX media_import_candidate ON media(file_name,file_size,id) WHERE state!='superseded';
CREATE TABLE import_jobs(
 id TEXT PRIMARY KEY,source_id TEXT NOT NULL REFERENCES import_sources(id),
 storage_root_id TEXT NOT NULL REFERENCES storage_roots(id),
 state TEXT NOT NULL CHECK(state IN('scanning','preview','importing','completed','cancelled','interrupted','failed')),
 stage TEXT NOT NULL DEFAULT 'preview' CHECK(stage IN('preview','import')),
 cancel_requested INTEGER NOT NULL DEFAULT 0 CHECK(cancel_requested IN(0,1)),
 created_at INTEGER NOT NULL,updated_at INTEGER NOT NULL,current_file TEXT,error TEXT,
 new_media INTEGER NOT NULL DEFAULT 0,existing_media INTEGER NOT NULL DEFAULT 0,
 duplicates INTEGER NOT NULL DEFAULT 0,new_tags INTEGER NOT NULL DEFAULT 0,
 conflicts INTEGER NOT NULL DEFAULT 0,required_bytes INTEGER NOT NULL DEFAULT 0
) STRICT;
CREATE INDEX import_job_work ON import_jobs(state,created_at,id);
CREATE TABLE import_entries(
 id INTEGER PRIMARY KEY,job_id TEXT NOT NULL REFERENCES import_jobs(id),relative_path TEXT NOT NULL,
 kind TEXT NOT NULL CHECK(kind IN('dir','file')),
 state TEXT NOT NULL CHECK(state IN('queued','ready','copying','done','failed')),
 file_name TEXT NOT NULL,file_size INTEGER NOT NULL DEFAULT 0,source_stat TEXT,full_hash TEXT,
 media_id TEXT,plan_kind TEXT CHECK(plan_kind IN('new','existing','duplicate')),
 needs_copy INTEGER NOT NULL DEFAULT 0 CHECK(needs_copy IN(0,1)),
 conflict INTEGER NOT NULL DEFAULT 0,target_path TEXT,result_kind TEXT,tags_added INTEGER NOT NULL DEFAULT 0,error TEXT,
 UNIQUE(job_id,relative_path)
) STRICT;
CREATE INDEX import_entry_work ON import_entries(job_id,kind,state,id);
CREATE INDEX import_entry_match ON import_entries(job_id,file_name,file_size,full_hash,id);
CREATE INDEX import_reserved_target ON import_entries(target_path) WHERE target_path IS NOT NULL;
CREATE TABLE import_plan_tags(job_id TEXT NOT NULL REFERENCES import_jobs(id),relative_dir TEXT NOT NULL,PRIMARY KEY(job_id,relative_dir)) STRICT;
CREATE TRIGGER import_tag_generation AFTER INSERT ON media_import_tags BEGIN
 UPDATE library_generation SET value=value+1 WHERE id=1;
END;
