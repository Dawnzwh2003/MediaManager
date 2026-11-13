//! Parameterized scanner persistence; no filesystem access or identity decisions.
use crate::{error::AppResult, identity::Evidence};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
pub(crate) const SCAN_BATCH: usize = 100;

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
pub(crate) struct Publication {
    pub observation: Observation,
    pub id: Option<String>,
    pub reason: &'static str,
    pub tags: Vec<String>,
}
pub(crate) struct ScanStore<'a>(pub &'a mut Connection);
pub(crate) struct ScanRun<'a> {
    pub root: &'a str,
    pub generation: i64,
    pub observed_at: i64,
}
const BY_PATH_SQL: &str = "SELECT m.id,m.file_path,e.stat,coalesce(m.fingerprint,''),m.missing FROM media m JOIN scan_evidence e ON e.media_id=m.id WHERE m.file_path=?1 AND m.state!='superseded' ORDER BY m.missing LIMIT 1";
const UNSEEN_SQL: &str = "SELECT e.media_id,m.file_path FROM scan_evidence e INDEXED BY scan_unseen JOIN media m ON m.id=e.media_id WHERE e.root=?1 AND e.generation!=?2 AND e.media_id>?3 AND m.state='active' ORDER BY e.media_id LIMIT 1";
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
    pub fn enqueue_batch(
        &mut self,
        root: &str,
        entries: &[(String, &'static str)],
    ) -> AppResult<()> {
        bounded(entries.len())?;
        let tx = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        for (path, kind) in entries {
            tx.execute(
                "INSERT OR IGNORE INTO scan_queue(root,path,kind) VALUES(?1,?2,?3)",
                params![root, path, kind],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn files(&self, root: &str) -> AppResult<Vec<String>> {
        Ok(self.0.prepare("SELECT path FROM scan_queue WHERE root=?1 AND kind='file' AND done=0 ORDER BY path LIMIT 100")?.query_map([root],|r|r.get(0))?.collect::<Result<_,_>>()?)
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
        Ok(self.0.query_row(BY_PATH_SQL, [path], stored).optional()?)
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
    pub fn stage_batch(
        &mut self,
        root: &str,
        files: &[(String, Option<Observation>)],
    ) -> AppResult<()> {
        bounded(files.len())?;
        let tx = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        for (path, observation) in files {
            if let Some(o) = observation {
                tx.execute(
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
            }
            tx.execute(
                "UPDATE scan_queue SET done=1 WHERE root=?1 AND path=?2",
                params![root, path],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn observation(&self, root: &str, after: &str) -> AppResult<Option<Observation>> {
        Ok(self.0.query_row("SELECT path,file_key,stat,fingerprint,size,modified,created,kind FROM scan_observations WHERE root=?1 AND path>?2 ORDER BY path LIMIT 1",params![root,after],|r| Ok(Observation {path:r.get(0)?,evidence:Evidence{key:r.get(1)?,stat:r.get(2)?,size:r.get(4)?,modified:r.get(5)?,created:r.get(6)?},fingerprint:r.get(3)?,kind:r.get(7)?})).optional()?)
    }
    pub fn persist_batch(
        &mut self,
        run: &ScanRun<'_>,
        publications: &[Publication],
    ) -> AppResult<()> {
        bounded(publications.len())?;
        let ScanRun {
            root,
            generation,
            observed_at: now,
        } = *run;
        let tx = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        // Generation guard also rejects delayed workers from an interrupted scan.
        let valid:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM scan_roots WHERE path=?1 AND generation=?2 AND state='running')",params![root,generation],|r|r.get(0))?;
        if !valid {
            return Err(crate::error::AppError::Busy);
        }
        for publication in publications {
            let o = &publication.observation;
            let id = publication.id.as_deref();
            let reason = publication.reason;
            let tags = &publication.tags;
            // A confirmed source move can replace a different active destination.
            // Retire that logical instance before reserving the path for the source.
            tx.execute("UPDATE media SET state='superseded',missing=1 WHERE file_path=?1 AND state='active' AND (?2 IS NULL OR id!=?2)",params![o.path,id])?;
            if let Some(id) = id {
                tx.execute("UPDATE media SET file_path=?2,file_name=?3,last_seen_at=?4,file_size=?5,modified_time=?6,created_time=?7,missing=0,state='active' WHERE id=?1",params![id,o.path,std::path::Path::new(&o.path).file_name().unwrap().to_str().unwrap(),now,o.evidence.size,o.evidence.modified,o.evidence.created])?;
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
        Ok(self
            .0
            .query_row(UNSEEN_SQL, params![root, generation, after], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .optional()?)
    }
    pub fn missing_batch(&mut self, ids: &[String]) -> AppResult<()> {
        bounded(ids.len())?;
        let tx = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        for id in ids {
            tx.execute(
                "UPDATE media SET state='missing',missing=1 WHERE id=?1",
                [id],
            )?;
            tx.execute(
                "UPDATE scan_evidence SET reason='MISSING' WHERE media_id=?1",
                [id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
}
fn bounded(len: usize) -> AppResult<()> {
    if len > SCAN_BATCH {
        return Err(crate::error::AppError::Validation(
            "scanner batch exceeds 100".into(),
        ));
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prepared_batches_roll_back_queue_and_publication_on_failure() {
        let temp = tempfile::tempdir().unwrap();
        let mut c = Connection::open_in_memory().unwrap();
        c.execute_batch("PRAGMA foreign_keys=ON").unwrap();
        crate::db::migrations::migrate(&mut c, temp.path()).unwrap();
        let mut store = ScanStore(&mut c);
        let generation = store.begin("/fixture").unwrap();
        store
            .directory("/fixture", "/fixture", None, "fixture", 1)
            .unwrap();
        store
            .enqueue_batch(
                "/fixture",
                &[
                    ("/fixture/a.jpg".into(), "file"),
                    ("/fixture/b.jpg".into(), "file"),
                ],
            )
            .unwrap();
        let observation = |name: &str| Observation {
            path: format!("/fixture/{name}.jpg"),
            evidence: Evidence {
                key: Some(name.into()),
                stat: name.into(),
                size: 1,
                modified: 1,
                created: None,
            },
            fingerprint: name.into(),
            kind: "image".into(),
        };
        store.0.execute_batch("CREATE TRIGGER reject_second BEFORE INSERT ON scan_observations WHEN NEW.path='/fixture/b.jpg' BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
        assert!(store
            .stage_batch(
                "/fixture",
                &[
                    ("/fixture/a.jpg".into(), Some(observation("a"))),
                    ("/fixture/b.jpg".into(), Some(observation("b")))
                ]
            )
            .is_err());
        let (observations, done): (i64, i64) = store.0.query_row("SELECT (SELECT count(*) FROM scan_observations),(SELECT count(*) FROM scan_queue WHERE done=1)", [], |r| Ok((r.get(0)?,r.get(1)?))).unwrap();
        assert_eq!((observations, done), (0, 0));
        store.0.execute_batch("DROP TRIGGER reject_second").unwrap();
        store
            .stage_batch(
                "/fixture",
                &[
                    ("/fixture/a.jpg".into(), Some(observation("a"))),
                    ("/fixture/b.jpg".into(), Some(observation("b"))),
                ],
            )
            .unwrap();
        let run = ScanRun {
            root: "/fixture",
            generation,
            observed_at: 1,
        };
        let mut publications = vec![
            Publication {
                observation: observation("a"),
                id: None,
                reason: "fixture",
                tags: vec!["/fixture".into()],
            },
            Publication {
                observation: observation("b"),
                id: None,
                reason: "fixture",
                tags: vec!["/absent".into()],
            },
        ];
        assert!(store.persist_batch(&run, &publications).is_err());
        assert_eq!(
            store
                .0
                .query_row("SELECT count(*) FROM media", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            0
        );
        publications[1].tags = vec!["/fixture".into()];
        store.persist_batch(&run, &publications).unwrap();
        assert_eq!(
            store
                .0
                .query_row("SELECT count(*) FROM media", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            2
        );
        store.begin("/fixture").unwrap();
        assert!(store.persist_batch(&run, &publications).is_err());
        assert!(store
            .enqueue_batch(
                "/fixture",
                &vec![("/fixture/extra.jpg".into(), "file"); SCAN_BATCH + 1]
            )
            .is_err());
    }
    fn scale(size: i64) {
        let temp = tempfile::tempdir().unwrap();
        let mut c = Connection::open(temp.path().join("scanner.sqlite3")).unwrap();
        c.execute_batch(
            "PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA cache_size=-2048;",
        )
        .unwrap();
        crate::db::migrations::migrate(&mut c, temp.path()).unwrap();
        c.execute_batch("BEGIN;").unwrap();
        c.execute("WITH RECURSIVE n(v) AS(VALUES(1) UNION ALL SELECT v+1 FROM n WHERE v<?1)
          INSERT INTO media(id,file_path,file_name,media_type,file_size,modified_time,imported_at,last_seen_at,created_seq)
          SELECT printf('m%06d',v),printf('/synthetic/%06d.jpg',v),printf('%06d.jpg',v),'image',1,1,v,1,v FROM n",[size]).unwrap();
        c.execute_batch("INSERT INTO scan_roots VALUES('/synthetic',1,'complete');
          INSERT INTO scan_evidence SELECT id,'/synthetic',id,'test',1,'test' FROM media;
          INSERT INTO scan_observations SELECT '/synthetic',file_path,id,'test','test',1,1,1,'image' FROM media;
          INSERT INTO scan_queue(root,path,kind) SELECT '/synthetic',file_path,'file' FROM media;
          UPDATE media SET state='missing',missing=1 WHERE created_seq%10=0;
          COMMIT;").unwrap();
        let started = std::time::Instant::now();
        let store = ScanStore(&mut c);
        let mut after = String::new();
        let mut count = 0;
        while let Some((id, _)) = store.unseen("/synthetic", 2, &after).unwrap() {
            assert!(id > after);
            after = id;
            count += 1;
        }
        assert_eq!(count, size - size / 10);
        let unseen_ms = started.elapsed().as_secs_f64() * 1000.0;
        let started = std::time::Instant::now();
        for n in 1..=1000 {
            let key = format!("/synthetic/{:06}.jpg", n * size / 1000);
            assert!(store.by_path(&key).unwrap().is_some());
        }
        let lookup_ms = started.elapsed().as_secs_f64() * 1000.0;
        let mut after = String::new();
        let mut observed = 0;
        while let Some(item) = store.observation("/synthetic", &after).unwrap() {
            after = item.path;
            observed += 1;
        }
        assert_eq!(observed, size);
        // Simulate queue consumption in a fixture-only transaction; production
        // bounded production transactions are exercised by actual scanner tests.
        store.0.execute_batch("BEGIN;").unwrap();
        for _ in 0..1000 {
            let path = store.next("/synthetic", "file").unwrap().unwrap();
            store.done("/synthetic", &path).unwrap();
        }
        store.0.execute_batch("ROLLBACK;").unwrap();
        assert_eq!(store.roots().unwrap().len(), 1);
        println!("SCANNER_STORAGE size={size} unseen_rows={count} unseen_ms={unseen_ms:.3} lookup_1000_ms={lookup_ms:.3} observed={observed}; only one Stored/Observation retained at a time");
    }
    #[test]
    fn scanner_storage_10000_is_streamed() {
        scale(10000)
    }
    #[test]
    fn scanner_storage_100000_is_streamed() {
        scale(100000)
    }
    #[test]
    fn scanner_path_and_unseen_queries_seek_without_scanning_or_sorting_the_root() {
        let temp = tempfile::tempdir().unwrap();
        let mut c = Connection::open_in_memory().unwrap();
        crate::db::migrations::migrate(&mut c, temp.path()).unwrap();
        for (sql, args) in [
            (BY_PATH_SQL, vec!["/synthetic/last.jpg", "", ""]),
            (UNSEEN_SQL, vec!["/synthetic", "2", "m050000"]),
        ] {
            let params = if sql == BY_PATH_SQL {
                vec![rusqlite::types::Value::Text(args[0].into())]
            } else {
                vec![
                    rusqlite::types::Value::Text(args[0].into()),
                    rusqlite::types::Value::Integer(2),
                    rusqlite::types::Value::Text(args[2].into()),
                ]
            };
            let lines = c
                .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
                .unwrap()
                .query_map(rusqlite::params_from_iter(params), |r| {
                    r.get::<_, String>(3)
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            assert!(
                !lines
                    .iter()
                    .any(|p| p.contains("SCAN m") || p.contains("TEMP B-TREE")),
                "{lines:?}"
            );
            assert!(lines.iter().any(|p| p.contains("SEARCH")), "{lines:?}");
        }
    }
}
