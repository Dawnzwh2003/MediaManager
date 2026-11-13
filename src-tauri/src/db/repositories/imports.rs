//! Import persistence only: no filesystem traversal, hashing or duplicate policy.
use crate::{
    domain::{importing::*, library::NewMedia},
    error::{AppError, AppResult},
    identity::Evidence,
};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

pub(crate) struct ImportStore<'a>(pub &'a mut Connection);
#[derive(Debug, Clone)]
pub(crate) struct JobContext {
    pub id: String,
    pub source_id: String,
    pub source: String,
    pub root_id: String,
    pub root: String,
    pub state: String,
    pub stage: String,
}
#[derive(Debug, Clone)]
pub(crate) struct Entry {
    pub id: i64,
    pub relative: String,
    pub name: String,
    pub size: i64,
    pub stat: Option<String>,
    pub hash: Option<String>,
    pub media_id: Option<String>,
    pub target: Option<String>,
}
pub(crate) struct Plan<'a> {
    pub entry: i64,
    pub size: i64,
    pub stat: &'a str,
    pub hash: &'a str,
    pub media_id: &'a str,
    pub kind: &'a str,
    pub copy: bool,
    pub conflict: bool,
    pub directories: &'a [String],
}
pub(crate) struct Physical {
    pub root_id: String,
    pub root: String,
    pub path: String,
    pub relative: String,
    pub evidence: Evidence,
}
pub(crate) struct Publication {
    pub media_id: String,
    pub new_media: Option<NewMedia>,
    pub expected_path: Option<String>,
    pub expected_version: i64,
    pub physical: Option<Physical>,
    pub hash: String,
    pub stat: String,
    pub directories: Vec<String>,
    pub now: i64,
}

fn entry(r: &rusqlite::Row<'_>) -> rusqlite::Result<Entry> {
    Ok(Entry {
        id: r.get(0)?,
        relative: r.get(1)?,
        name: r.get(2)?,
        size: r.get(3)?,
        stat: r.get(4)?,
        hash: r.get(5)?,
        media_id: r.get(6)?,
        target: r.get(7)?,
    })
}
impl ImportStore<'_> {
    pub fn id(&self) -> AppResult<String> {
        Ok(self
            .0
            .query_row("SELECT lower(hex(randomblob(16)))", [], |r| r.get(0))?)
    }
    pub fn root(&self, path: &str, now: i64) -> AppResult<String> {
        let id = self.id()?;
        self.0.execute(
            "INSERT OR IGNORE INTO storage_roots VALUES(?1,?2,?3)",
            params![id, path, now],
        )?;
        Ok(self
            .0
            .query_row("SELECT id FROM storage_roots WHERE path=?1", [path], |r| {
                r.get(0)
            })?)
    }
    pub fn roots(&self) -> AppResult<Vec<String>> {
        Ok(self
            .0
            .prepare("SELECT path FROM storage_roots")?
            .query_map([], |r| r.get(0))?
            .collect::<Result<_, _>>()?)
    }
    pub fn scan_roots(&self) -> AppResult<Vec<String>> {
        Ok(self
            .0
            .prepare("SELECT path FROM scan_roots")?
            .query_map([], |r| r.get(0))?
            .collect::<Result<_, _>>()?)
    }
    pub fn sources(&self) -> AppResult<Vec<String>> {
        Ok(self
            .0
            .prepare("SELECT root_path FROM import_sources")?
            .query_map([], |r| r.get(0))?
            .collect::<Result<_, _>>()?)
    }
    pub fn create(&mut self, source: &str, label: &str, root: &str, now: i64) -> AppResult<String> {
        let id = self.id()?;
        let source_id = self.id()?;
        let tx = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if tx.query_row("SELECT EXISTS(SELECT 1 FROM import_jobs WHERE state IN('scanning','preview','importing','interrupted'))",[],|r|r.get::<_,bool>(0))? { return Err(AppError::Busy); }
        tx.execute(
            "INSERT OR IGNORE INTO import_sources VALUES(?1,?2,?3,?4)",
            params![source_id, source, label, now],
        )?;
        tx.execute("INSERT INTO import_jobs(id,source_id,storage_root_id,state,created_at,updated_at) SELECT ?1,id,?2,'scanning',?3,?3 FROM import_sources WHERE root_path=?4",params![id,root,now,source])?;
        tx.execute("INSERT INTO import_entries(job_id,relative_path,kind,state,file_name) VALUES(?1,'','dir','queued','')",[&id])?;
        tx.commit()?;
        Ok(id)
    }
    pub fn context(&self, id: &str) -> AppResult<JobContext> {
        self.0.query_row("SELECT j.id,j.source_id,s.root_path,j.storage_root_id,r.path,j.state,j.stage FROM import_jobs j JOIN import_sources s ON s.id=j.source_id JOIN storage_roots r ON r.id=j.storage_root_id WHERE j.id=?1",[id],|r|Ok(JobContext{id:r.get(0)?,source_id:r.get(1)?,source:r.get(2)?,root_id:r.get(3)?,root:r.get(4)?,state:r.get(5)?,stage:r.get(6)?})).optional()?.ok_or(AppError::NotFound)
    }
    pub fn latest(&self) -> AppResult<Option<String>> {
        Ok(self
            .0
            .query_row(
                "SELECT id FROM import_jobs ORDER BY created_at DESC,rowid DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()?)
    }
    pub fn work(&self) -> AppResult<Option<String>> {
        Ok(self.0.query_row("SELECT id FROM import_jobs WHERE state IN('scanning','importing') ORDER BY created_at,id LIMIT 1",[],|r|r.get(0)).optional()?)
    }
    pub fn recovery_boundary(&self) -> AppResult<i64> {
        Ok(self
            .0
            .query_row("SELECT coalesce(max(rowid),0) FROM import_jobs", [], |r| {
                r.get(0)
            })?)
    }
    pub fn recover(&self, boundary: i64) -> AppResult<()> {
        self.0.execute("UPDATE import_jobs SET state='interrupted' WHERE state IN('scanning','importing') AND rowid<=?1",[boundary])?;
        Ok(())
    }
    pub fn set_state(&self, id: &str, state: &str, error: Option<&str>) -> AppResult<()> {
        self.0.execute("UPDATE import_jobs SET state=?2,error=?3,current_file=NULL,updated_at=CAST(strftime('%s','now') AS INTEGER)*1000 WHERE id=?1",params![id,state,error])?;
        Ok(())
    }
    pub fn current(&self, id: &str, path: &str) -> AppResult<()> {
        self.0.execute(
            "UPDATE import_jobs SET current_file=?2 WHERE id=?1",
            params![id, path],
        )?;
        Ok(())
    }
    pub fn cancelled(&self, id: &str) -> AppResult<bool> {
        Ok(self.0.query_row(
            "SELECT cancel_requested FROM import_jobs WHERE id=?1",
            [id],
            |r| r.get(0),
        )?)
    }
    pub fn cancel(&self, id: &str) -> AppResult<()> {
        self.0.execute("UPDATE import_jobs SET cancel_requested=1,state=CASE WHEN state IN('preview','interrupted') THEN 'cancelled' ELSE state END WHERE id=?1 AND state NOT IN('completed','failed','cancelled')",[id])?;
        Ok(())
    }
    pub fn start(&mut self, id: &str, resume: bool) -> AppResult<()> {
        let job = self.context(id)?;
        if (!resume && job.state != "preview")
            || (resume
                && !matches!(
                    job.state.as_str(),
                    "cancelled" | "interrupted" | "failed" | "completed"
                ))
        {
            return Err(AppError::Validation("job is not ready".into()));
        }
        let unplanned_failure: bool = self.0.query_row("SELECT EXISTS(SELECT 1 FROM import_entries WHERE job_id=?1 AND state='failed' AND full_hash IS NULL)",[id],|r|r.get(0))?;
        let state = if resume && (job.stage == "preview" || unplanned_failure) {
            "scanning"
        } else {
            "importing"
        };
        let tx = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if tx.query_row("SELECT EXISTS(SELECT 1 FROM import_jobs WHERE id!=?1 AND state IN('scanning','preview','importing','interrupted'))",[id],|r|r.get::<_,bool>(0))? { return Err(AppError::Busy); }
        let current: String =
            tx.query_row("SELECT state FROM import_jobs WHERE id=?1", [id], |r| {
                r.get(0)
            })?;
        if current != job.state {
            return Err(AppError::StaleState);
        }
        if resume {
            // Keep durable media IDs/target reservations so a promoted file can be published after a DB failure.
            tx.execute("UPDATE import_entries SET state=CASE WHEN full_hash IS NULL THEN 'queued' ELSE 'ready' END,error=NULL WHERE job_id=?1 AND state='failed' AND (?2='scanning' OR full_hash IS NOT NULL)",params![id,state])?;
        }
        tx.execute("UPDATE import_jobs SET state=?2,stage=CASE WHEN ?2='importing' THEN 'import' ELSE 'preview' END,cancel_requested=0,error=NULL WHERE id=?1",params![id,state])?;
        tx.commit()?;
        Ok(())
    }
    pub fn next(&self, job: &str, kind: &str, preview: bool) -> AppResult<Option<Entry>> {
        let states = if preview {
            "('queued')"
        } else {
            "('ready','copying')"
        };
        Ok(self.0.query_row(&format!("SELECT id,relative_path,file_name,file_size,source_stat,full_hash,media_id,target_path FROM import_entries WHERE job_id=?1 AND kind=?2 AND state IN {states} ORDER BY id LIMIT 1"),params![job,kind],entry).optional()?)
    }
    pub fn enqueue(&mut self, job: &str, rows: &[(String, String, String)]) -> AppResult<()> {
        if rows.len() > 100 {
            return Err(AppError::Validation(
                "import queue batch exceeds 100".into(),
            ));
        }
        let tx = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        for (relative, kind, name) in rows {
            tx.execute("INSERT OR IGNORE INTO import_entries(job_id,relative_path,kind,state,file_name) VALUES(?1,?2,?3,'queued',?4)",params![job,relative,kind,name])?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn finish_directory(&self, id: i64) -> AppResult<()> {
        self.0.execute(
            "UPDATE import_entries SET state='done' WHERE id=?1 AND state!='failed'",
            [id],
        )?;
        Ok(())
    }
    pub fn fail_entry(&self, id: i64, error: &str) -> AppResult<()> {
        self.0.execute(
            "UPDATE import_entries SET state='failed',error=?2 WHERE id=?1",
            params![id, error],
        )?;
        Ok(())
    }
    pub fn plan(&mut self, job: &str, p: Plan<'_>) -> AppResult<()> {
        let tx = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute("UPDATE import_entries SET state='ready',file_size=?2,source_stat=?3,full_hash=?4,media_id=?5,plan_kind=?6,needs_copy=?7,conflict=?8 WHERE id=?1",params![p.entry,p.size,p.stat,p.hash,p.media_id,p.kind,p.copy,p.conflict])?;
        for d in p.directories {
            tx.execute(
                "INSERT OR IGNORE INTO import_plan_tags VALUES(?1,?2)",
                params![job, d],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn planned_match(
        &self,
        job: &str,
        name: &str,
        size: i64,
        hash: &str,
    ) -> AppResult<Option<String>> {
        Ok(self.0.query_row("SELECT media_id FROM import_entries WHERE job_id=?1 AND file_name=?2 AND file_size=?3 AND full_hash=?4 AND state IN('ready','copying','done') ORDER BY id LIMIT 1",params![job,name,size,hash],|r|r.get(0)).optional()?)
    }
    pub fn name_exists(&self, job: &str, name: &str) -> AppResult<bool> {
        Ok(self.0.query_row("SELECT EXISTS(SELECT 1 FROM media WHERE file_name=?2 AND state!='superseded') OR EXISTS(SELECT 1 FROM import_entries WHERE job_id=?1 AND file_name=?2 AND state='ready')",params![job,name],|r|r.get(0))?)
    }
    pub fn candidates(&self, name: &str, size: i64, after: &str) -> AppResult<Vec<String>> {
        Ok(self.0.prepare("SELECT id FROM media INDEXED BY media_import_candidate WHERE state!='superseded' AND file_name=?1 AND file_size=?2 AND id>?3 ORDER BY id LIMIT 50")?.query_map(params![name,size,after],|r|r.get(0))?.collect::<Result<_,_>>()?)
    }
    pub fn managed(&self, id: &str) -> AppResult<Option<(String, String)>> {
        Ok(self.0.query_row("SELECT r.id,r.path FROM managed_media m JOIN storage_roots r ON r.id=m.storage_root_id WHERE m.media_id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?)
    }
    pub fn evidence(&self, id: &str) -> AppResult<Option<String>> {
        Ok(self
            .0
            .query_row(
                "SELECT stat FROM scan_evidence WHERE media_id=?1",
                [id],
                |r| r.get(0),
            )
            .optional()?)
    }
    pub fn full_hash(&self, id: &str, version: i64) -> AppResult<Option<String>> {
        Ok(self.0.query_row("SELECT full_hash FROM media_full_hashes WHERE media_id=?1 AND content_version_id=?2",params![id,version],|r|r.get(0)).optional()?)
    }
    pub fn preview_ready(&self, job: &str) -> AppResult<()> {
        self.0.execute("UPDATE import_jobs SET state='preview',current_file=NULL,
   new_media=(SELECT count(*) FROM import_entries WHERE job_id=?1 AND plan_kind='new'),
   existing_media=(SELECT count(*) FROM import_entries WHERE job_id=?1 AND plan_kind='existing'),
   duplicates=(SELECT count(*) FROM import_entries WHERE job_id=?1 AND plan_kind IN('existing','duplicate')),
   conflicts=(SELECT count(*) FROM import_entries WHERE job_id=?1 AND conflict=1),
   required_bytes=(SELECT coalesce(sum(bytes),0) FROM (SELECT max(file_size) bytes FROM import_entries WHERE job_id=?1 AND needs_copy=1 AND plan_kind!='duplicate' AND state!='done' GROUP BY media_id)),
   new_tags=(SELECT count(*) FROM import_plan_tags p WHERE p.job_id=?1 AND NOT EXISTS(SELECT 1 FROM import_source_tags t WHERE t.source_id=import_jobs.source_id AND t.relative_dir=p.relative_dir)) WHERE id=?1",[job])?;
        Ok(())
    }
    pub fn reserve(&self, id: i64, path: &str) -> AppResult<()> {
        self.0.execute(
            "UPDATE import_entries SET state='copying',target_path=?2 WHERE id=?1",
            params![id, path],
        )?;
        Ok(())
    }
    pub fn snapshot(&self, id: &str) -> AppResult<ImportJob> {
        let mut job=self.0.query_row("SELECT j.id,s.root_path,r.path,j.state,j.stage,j.new_media,j.existing_media,j.duplicates,j.new_tags,j.conflicts,j.required_bytes,j.current_file,j.error FROM import_jobs j JOIN import_sources s ON s.id=j.source_id JOIN storage_roots r ON r.id=j.storage_root_id WHERE j.id=?1",[id],|r|Ok(ImportJob{id:r.get(0)?,source_root:r.get(1)?,storage_root:r.get(2)?,state:r.get(3)?,stage:r.get(4)?,new_media:r.get(5)?,existing_media:r.get(6)?,duplicate_files:r.get(7)?,new_tags:r.get(8)?,conflicts:r.get(9)?,estimated_storage_required:r.get(10)?,current_file:r.get(11)?,error:r.get(12)?,scanned_files:0,processed:0,newly_imported:0,existing_media_reused:0,tags_added:0,failed_files:0,failures:vec![]}))?;
        let counts:(i64,i64,i64,i64,i64,i64)=self.0.query_row("SELECT count(*),coalesce(sum(state IN('done','failed')),0),coalesce(sum(result_kind='new'),0),coalesce(sum(result_kind='reused'),0),coalesce(sum(tags_added),0),coalesce(sum(state='failed'),0) FROM import_entries WHERE job_id=?1 AND kind='file'",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?)))?;
        (
            job.scanned_files,
            job.processed,
            job.newly_imported,
            job.existing_media_reused,
            job.tags_added,
            job.failed_files,
        ) = counts;
        job.failed_files = self.0.query_row(
            "SELECT count(*) FROM import_entries WHERE job_id=?1 AND state='failed'",
            [id],
            |r| r.get(0),
        )?;
        job.failures=self.0.prepare("SELECT relative_path,error FROM import_entries WHERE job_id=?1 AND state='failed' ORDER BY id LIMIT 20")?.query_map([id],|r|Ok(ImportFailure{relative_path:r.get(0)?,error:r.get(1)?}))?.collect::<Result<_,_>>()?;
        Ok(job)
    }
    pub fn publish(&mut self, job: &JobContext, item: &Entry, p: Publication) -> AppResult<()> {
        let tx = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let version = if let Some(input) = &p.new_media {
            super::MediaRepository::insert_in(&tx, input)?;
            1
        } else {
            let (version,path):(i64,String)=tx.query_row("SELECT current_content_version_id,file_path FROM media WHERE id=?1 AND state!='superseded'",[&p.media_id],|r|Ok((r.get(0)?,r.get(1)?)))?;
            if version != p.expected_version || p.expected_path.as_deref() != Some(&path) {
                return Err(AppError::StaleState);
            }
            version
        };
        if let Some(physical) = &p.physical {
            if p.new_media.is_none() {
                tx.execute("UPDATE media SET file_path=?2,missing=0,state='active',last_seen_at=?3 WHERE id=?1",params![p.media_id,physical.path,p.now])?;
            }
            tx.execute(
                "INSERT OR IGNORE INTO scan_roots(path,state) VALUES(?1,'complete')",
                [&physical.root],
            )?;
            let parent = std::path::Path::new(&physical.path)
                .parent()
                .ok_or(AppError::Internal)?
                .to_str()
                .ok_or(AppError::Internal)?;
            tx.execute("INSERT OR IGNORE INTO system_tags(id,parent_id,source_root,directory_path,name,last_seen_at) VALUES(?1,NULL,?1,?1,?2,?3)",params![physical.root,std::path::Path::new(&physical.root).file_name().and_then(|s|s.to_str()).unwrap_or("Media"),p.now])?;
            let root_tag: String = tx.query_row(
                "SELECT id FROM system_tags WHERE directory_path=?1",
                [&physical.root],
                |r| r.get(0),
            )?;
            tx.execute("INSERT OR IGNORE INTO system_tags(id,parent_id,source_root,directory_path,name,last_seen_at) VALUES(?1,?2,?3,?1,?4,?5)",params![parent,root_tag,physical.root,p.media_id,p.now])?;
            let leaf: String = tx.query_row(
                "SELECT id FROM system_tags WHERE directory_path=?1",
                [parent],
                |r| r.get(0),
            )?;
            tx.execute(
                "DELETE FROM media_system_tags WHERE media_id=?1",
                [&p.media_id],
            )?;
            for tag in [&root_tag, &leaf] {
                tx.execute(
                    "INSERT OR IGNORE INTO media_system_tags VALUES(?1,?2)",
                    params![p.media_id, tag],
                )?;
            }
            tx.execute("INSERT INTO scan_evidence(media_id,root,file_key,stat,generation,reason) SELECT ?1,?2,?3,?4,generation,'FOLDER_IMPORT' FROM scan_roots WHERE path=?2 ON CONFLICT(media_id) DO UPDATE SET root=excluded.root,file_key=excluded.file_key,stat=excluded.stat,generation=excluded.generation,reason=excluded.reason",params![p.media_id,physical.root,physical.evidence.key,physical.evidence.stat])?;
            tx.execute("INSERT INTO managed_media VALUES(?1,?2,?3) ON CONFLICT(media_id) DO UPDATE SET storage_root_id=excluded.storage_root_id,relative_path=excluded.relative_path",params![p.media_id,physical.root_id,physical.relative])?;
        }
        tx.execute("INSERT INTO media_full_hashes VALUES(?1,?2,'sha256-v1',?3,?4,?5) ON CONFLICT(media_id,content_version_id) DO UPDATE SET full_hash=excluded.full_hash,file_size=excluded.file_size,stat=excluded.stat",params![p.media_id,version,p.hash,item.size,p.stat])?;
        tx.execute("INSERT INTO media_import_sources VALUES(?1,?2,?3,?4,?4) ON CONFLICT(media_id,source_id,relative_path) DO UPDATE SET last_seen_at=excluded.last_seen_at",params![p.media_id,job.source_id,item.relative,p.now])?;
        let mut parent_id: Option<String> = None;
        let mut added = 0;
        for d in &p.directories {
            let id: String = tx.query_row("SELECT lower(hex(randomblob(16)))", [], |r| r.get(0))?;
            let name = d.rsplit('/').next().ok_or(AppError::Internal)?;
            tx.execute(
                "INSERT OR IGNORE INTO import_source_tags VALUES(?1,?2,?3,?4,?5)",
                params![id, job.source_id, parent_id, d, name],
            )?;
            let tag: String = tx.query_row(
                "SELECT id FROM import_source_tags WHERE source_id=?1 AND relative_dir=?2",
                params![job.source_id, d],
                |r| r.get(0),
            )?;
            added += tx.execute(
                "INSERT OR IGNORE INTO media_import_tags VALUES(?1,?2)",
                params![p.media_id, tag],
            )?;
            parent_id = Some(tag);
        }
        tx.execute("UPDATE import_entries SET state='done',result_kind=?2,tags_added=?3,error=NULL WHERE id=?1",params![item.id,if p.new_media.is_some(){"new"}else{"reused"},added as i64])?;
        tx.commit()?;
        Ok(())
    }
    pub fn source_nodes(
        &self,
        source: Option<&str>,
        parent: Option<&str>,
        after: &str,
    ) -> AppResult<SourcePage> {
        let mut items: Vec<SourceNode> = if let Some(source) = source {
            self.0.prepare("SELECT id,name,source_id,parent_id,relative_dir,EXISTS(SELECT 1 FROM import_source_tags c WHERE c.parent_id=t.id) FROM import_source_tags t WHERE source_id=?1 AND parent_id IS ?2 AND id>?3 ORDER BY id LIMIT 101")?.query_map(params![source,parent,after],|r|Ok(SourceNode{id:r.get(0)?,name:r.get(1)?,source_id:r.get(2)?,parent_id:r.get(3)?,relative_path:r.get(4)?,has_children:r.get(5)?,root:false}))?.collect::<Result<_,_>>()?
        } else {
            self.0.prepare("SELECT id,label,root_path,EXISTS(SELECT 1 FROM import_source_tags t WHERE t.source_id=s.id) FROM import_sources s WHERE id>?1 ORDER BY id LIMIT 101")?.query_map([after],|r|{let id:String=r.get(0)?;Ok(SourceNode{id:id.clone(),source_id:id,name:r.get(1)?,relative_path:r.get(2)?,parent_id:None,has_children:r.get(3)?,root:true})})?.collect::<Result<_,_>>()?
        };
        let more = items.len() > 100;
        items.truncate(100);
        let next_after = if more {
            items.last().map(|i| i.id.clone())
        } else {
            None
        };
        Ok(SourcePage { items, next_after })
    }
}

impl crate::db::Database {
    pub(crate) fn import_store(&mut self) -> ImportStore<'_> {
        ImportStore(&mut self.connection)
    }
}
