//! Parameterized scanner persistence; no filesystem access or identity decisions.
use crate::{error::AppResult, identity::Evidence};
use rusqlite::{params, Connection, OptionalExtension};

#[derive(Debug)]
pub(crate) struct Stored {
    pub id: String,
    pub path: String,
    pub stat: String,
    pub fingerprint: String,
    pub missing: bool,
}
#[derive(Debug)]
pub(crate) struct Observation {
    pub path: String,
    pub evidence: Evidence,
    pub fingerprint: String,
    pub kind: String,
}
pub(crate) struct ScanStore<'a>(pub &'a mut Connection);
impl ScanStore<'_> {
    pub fn roots(&self) -> AppResult<Vec<String>> {
        Ok(self
            .0
            .prepare("SELECT path FROM scan_roots")?
            .query_map([], |r| r.get(0))?
            .collect::<Result<_, _>>()?)
    }
    pub fn begin(&mut self, root: &str) -> AppResult<i64> {
        let tx = self.0.transaction()?;
        tx.execute(
            "UPDATE scan_roots SET state='interrupted' WHERE state='running'",
            [],
        )?;
        tx.execute("INSERT INTO scan_roots(path,state) VALUES(?1,'running') ON CONFLICT(path) DO UPDATE SET generation=generation+1,state='running'",[root])?;
        tx.execute("DELETE FROM scan_queue WHERE root=?1", [root])?;
        tx.execute("DELETE FROM scan_observations WHERE root=?1", [root])?;
        tx.execute(
            "INSERT INTO scan_queue(root,path,kind) VALUES(?1,?1,'dir')",
            [root],
        )?;
        let generation = tx.query_row(
            "SELECT generation FROM scan_roots WHERE path=?1",
            [root],
            |r| r.get(0),
        )?;
        tx.commit()?;
        Ok(generation)
    }
    pub fn state(&self, root: &str, state: &str) -> AppResult<()> {
        self.0.execute(
            "UPDATE scan_roots SET state=?2 WHERE path=?1",
            params![root, state],
        )?;
        Ok(())
    }
    pub fn enqueue(&self, root: &str, path: &str, kind: &str) -> AppResult<()> {
        self.0.execute(
            "INSERT OR IGNORE INTO scan_queue(root,path,kind) VALUES(?1,?2,?3)",
            params![root, path, kind],
        )?;
        Ok(())
    }
    pub fn next(&self, root: &str, kind: &str) -> AppResult<Option<String>> {
        Ok(self.0.query_row("SELECT path FROM scan_queue WHERE root=?1 AND kind=?2 AND done=0 ORDER BY path LIMIT 1",params![root,kind],|r|r.get(0)).optional()?)
    }
    pub fn done(&self, root: &str, path: &str) -> AppResult<()> {
        self.0.execute(
            "UPDATE scan_queue SET done=1 WHERE root=?1 AND path=?2",
            params![root, path],
        )?;
        Ok(())
    }
    pub fn directory(
        &self,
        root: &str,
        path: &str,
        parent: Option<&str>,
        name: &str,
        now: i64,
    ) -> AppResult<()> {
        self.0.execute("INSERT INTO system_tags(id,parent_id,source_root,directory_path,name,last_seen_at) VALUES(?1,?2,?3,?1,?4,?5) ON CONFLICT(directory_path) DO UPDATE SET last_seen_at=excluded.last_seen_at",params![path,parent,root,name,now])?;
        Ok(())
    }
    pub fn by_path(&self, path: &str) -> AppResult<Option<Stored>> {
        Ok(self.0.query_row("SELECT m.id,m.file_path,e.stat,coalesce(m.fingerprint,''),m.missing FROM media m JOIN scan_evidence e ON e.media_id=m.id WHERE m.file_path=?1 AND m.state!='superseded' ORDER BY m.missing LIMIT 1",[path],stored).optional()?)
    }
    pub fn candidates(&self, root: &str, key: &str) -> AppResult<Vec<Stored>> {
        Ok(self.0.prepare("SELECT m.id,m.file_path,e.stat,coalesce(m.fingerprint,''),m.missing FROM scan_evidence e JOIN media m ON m.id=e.media_id WHERE e.root=?1 AND e.file_key=?2 AND m.state!='superseded' LIMIT 2")?.query_map(params![root,key],stored)?.collect::<Result<_,_>>()?)
    }
    pub fn identity_count(&self, root: &str, key: &str) -> AppResult<i64> {
        Ok(self.0.query_row(
            "SELECT count(*) FROM scan_observations WHERE root=?1 AND file_key=?2",
            params![root, key],
            |r| r.get(0),
        )?)
    }
    pub fn stage(&self, root: &str, o: &Observation) -> AppResult<()> {
        self.0.execute(
            "INSERT INTO scan_observations VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                root,
                o.path,
                o.evidence.key,
                o.evidence.stat,
                o.fingerprint,
                o.evidence.size,
                o.evidence.modified,
                o.evidence.created,
                o.kind
            ],
        )?;
        Ok(())
    }
    pub fn observation(&self, root: &str, after: &str) -> AppResult<Option<Observation>> {
        Ok(self.0.query_row("SELECT path,file_key,stat,fingerprint,size,modified,created,kind FROM scan_observations WHERE root=?1 AND path>?2 ORDER BY path LIMIT 1",params![root,after],|r| Ok(Observation {path:r.get(0)?,evidence:Evidence{key:r.get(1)?,stat:r.get(2)?,size:r.get(4)?,modified:r.get(5)?,created:r.get(6)?},fingerprint:r.get(3)?,kind:r.get(7)?})).optional()?)
    }
    pub fn persist(
        &mut self,
        root: &str,
        generation: i64,
        o: &Observation,
        id: Option<&str>,
        reason: &str,
        now: i64,
        tags: &[String],
    ) -> AppResult<()> {
        let tx = self.0.transaction()?;
        // Generation guard also rejects delayed workers from an interrupted scan.
        let valid:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM scan_roots WHERE path=?1 AND generation=?2 AND state='running')",params![root,generation],|r|r.get(0))?;
        if !valid {
            return Err(crate::error::AppError::Busy);
        }
        if let Some(id) = id {
            tx.execute("UPDATE media SET file_path=?2,file_name=?3,last_seen_at=?4,file_size=?5,modified_time=?6,created_time=?7,missing=0,state='active' WHERE id=?1",params![id,o.path,std::path::Path::new(&o.path).file_name().unwrap().to_str().unwrap(),now,o.evidence.size,o.evidence.modified,o.evidence.created])?;
        } else {
            tx.execute("UPDATE media SET state='superseded',missing=1 WHERE file_path=?1 AND state='active'",[&o.path])?;
        }
        let id = match id {
            Some(id) => id.to_owned(),
            None => {
                // SQLite randomblob supplies opaque UUID v4 entropy; independent of path/digest.
                let raw: String =
                    tx.query_row("SELECT lower(hex(randomblob(16)))", [], |r| r.get(0))?;
                let id = format!(
                    "{}-{}-4{}-8{}-{}",
                    &raw[..8],
                    &raw[8..12],
                    &raw[13..16],
                    &raw[17..20],
                    &raw[20..]
                );
                let seq: i64 = tx.query_row(
                    "UPDATE db_sequence SET value=value+1 WHERE id=1 RETURNING value",
                    [],
                    |r| r.get(0),
                )?;
                tx.execute("INSERT INTO media(id,fingerprint,file_path,file_name,media_type,file_size,created_time,modified_time,imported_at,last_seen_at,created_seq) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?9,?10)",params![id,o.fingerprint,o.path,std::path::Path::new(&o.path).file_name().unwrap().to_str().unwrap(),o.kind,o.evidence.size,o.evidence.created,o.evidence.modified,now,seq])?;
                id
            }
        };
        tx.execute("INSERT INTO scan_evidence VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(media_id) DO UPDATE SET root=excluded.root,file_key=excluded.file_key,stat=excluded.stat,generation=excluded.generation,reason=excluded.reason",params![id,root,o.evidence.key,o.evidence.stat,generation,reason])?;
        tx.execute("DELETE FROM media_system_tags WHERE media_id=?1", [&id])?;
        for tag in tags {
            tx.execute(
                "INSERT INTO media_system_tags VALUES(?1,?2)",
                params![id, tag],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn unseen(
        &self,
        root: &str,
        generation: i64,
        after: &str,
    ) -> AppResult<Option<(String, String)>> {
        Ok(self.0.query_row("SELECT m.id,m.file_path FROM media m JOIN scan_evidence e ON e.media_id=m.id WHERE e.root=?1 AND e.generation!=?2 AND m.id>?3 AND m.state='active' ORDER BY m.id LIMIT 1",params![root,generation,after],|r|Ok((r.get(0)?,r.get(1)?))).optional()?)
    }
    pub fn missing(&mut self, id: &str) -> AppResult<()> {
        let tx = self.0.transaction()?;
        tx.execute(
            "UPDATE media SET state='missing',missing=1 WHERE id=?1",
            [id],
        )?;
        tx.execute(
            "UPDATE scan_evidence SET reason='MISSING' WHERE media_id=?1",
            [id],
        )?;
        tx.commit()?;
        Ok(())
    }
}
fn stored(r: &rusqlite::Row<'_>) -> rusqlite::Result<Stored> {
    Ok(Stored {
        id: r.get(0)?,
        path: r.get(1)?,
        stat: r.get(2)?,
        fingerprint: r.get(3)?,
        missing: r.get(4)?,
    })
}
