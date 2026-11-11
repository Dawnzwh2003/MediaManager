//! One seed per query connection. SQLite owns the disk-backed temporary rank index.
use crate::{
    domain::library_query::RandomOrder,
    error::{AppError, AppResult},
};
use rusqlite::{params, Connection, Transaction};
use sha2::{Digest, Sha256};

pub(super) fn is_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub(super) fn initialize(connection: &Connection, order: Option<&RandomOrder>) -> AppResult<()> {
    if !order.is_some_and(|order| order.version == 1 && is_hex(&order.seed, 32)) {
        return Err(AppError::Validation(
            "invalid random seed or algorithm version".into(),
        ));
    }
    let initialized: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_temp_master WHERE name='library_random_state')",
        [],
        |row| row.get(0),
    )?;
    if !initialized {
        connection.execute_batch("PRAGMA temp_store=FILE;
            PRAGMA temp.cache_size=-4096;
            CREATE TEMP TABLE library_random_order(media_id TEXT PRIMARY KEY,sort_key TEXT NOT NULL) WITHOUT ROWID;
            CREATE INDEX temp.library_random_rank ON library_random_order(sort_key,media_id);
            CREATE TEMP TABLE library_random_state(seed TEXT NOT NULL, through_seq INTEGER NOT NULL);
            INSERT INTO library_random_state VALUES('',0);")?;
    }
    Ok(())
}

pub(super) fn populate(tx: &Transaction<'_>, order: &RandomOrder) -> AppResult<()> {
    populate_while(tx, order, &|| false)
}

pub(super) fn populate_while(
    tx: &Transaction<'_>,
    order: &RandomOrder,
    cancelled: &dyn Fn() -> bool,
) -> AppResult<()> {
    let (cached_seed, mut through): (String, i64) = tx.query_row(
        "SELECT seed,through_seq FROM temp.library_random_state",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    if cached_seed != order.seed {
        tx.execute("DELETE FROM temp.library_random_order", [])?;
        through = 0;
    }
    let high: i64 = tx.query_row(
        "SELECT coalesce(max(created_seq),0) FROM media",
        [],
        |row| row.get(0),
    )?;
    let changed = cached_seed != order.seed || high > through;
    if high > through {
        let mut source = tx.prepare(
            "SELECT id FROM media WHERE created_seq>?1 AND created_seq<=?2 ORDER BY created_seq",
        )?;
        let mut rows = source.query(params![through, high])?;
        let mut insert = tx.prepare(
            "INSERT OR IGNORE INTO temp.library_random_order(media_id,sort_key) VALUES(?1,?2)",
        )?;
        let prefix = hash_prefix(&order.seed);
        while let Some(row) = rows.next()? {
            if cancelled() {
                return Err(AppError::Validation("snapshot_cancelled".into()));
            }
            let id = row.get_ref(0)?.as_str().map_err(|_| AppError::Internal)?;
            let mut hash = prefix.clone();
            hash_field(&mut hash, id.as_bytes());
            insert.execute(params![id, format!("{:x}", hash.finalize())])?;
        }
        through = high;
    }
    if changed {
        tx.execute(
            "UPDATE temp.library_random_state SET seed=?1,through_seq=?2",
            params![order.seed, through],
        )?;
    }
    Ok(())
}

fn hash_prefix(seed: &str) -> Sha256 {
    let mut hash = Sha256::new();
    hash_field(&mut hash, b"MediaManager/random/v1");
    hash_field(&mut hash, seed.as_bytes());
    hash
}

fn hash_field(hash: &mut Sha256, field: &[u8]) {
    hash.update((field.len() as u64).to_be_bytes());
    hash.update(field);
}
