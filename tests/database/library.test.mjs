import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import { test } from "node:test";

const first = readFileSync(new URL("../../src-tauri/migrations/0001_app_metadata.sql", import.meta.url), "utf8");
const model = readFileSync(new URL("../../src-tauri/migrations/0002_library_model.sql", import.meta.url), "utf8");

function fixture(run) {
  const db = new DatabaseSync(":memory:");
  try {
    db.exec("PRAGMA foreign_keys=ON; PRAGMA recursive_triggers=ON; BEGIN");
    db.exec(first);
    db.exec(model);
    db.exec("COMMIT");
    run(db);
    assert.deepEqual(db.prepare("PRAGMA foreign_key_check").all(), []);
  } finally { db.close(); }
}
function sequence(db) { return db.prepare("UPDATE db_sequence SET value=value+1 WHERE id=1 RETURNING value").get().value; }
function media(db, id = "m1", path = `/${id}.jpg`, fingerprint = "same-content") {
  db.prepare("INSERT INTO media(id,fingerprint,file_path,file_name,media_type,file_size,modified_time,imported_at,last_seen_at,created_seq) VALUES(?,?,?,?,'image',42,10,20,30,?)")
    .run(id, fingerprint, path, path.split("/").at(-1), sequence(db));
}
function like(db, id = "l1", mediaId = "m1", request = id) {
  db.prepare("INSERT INTO like_events(id,ingest_seq,request_id,media_id,occurred_at,recorded_at) VALUES(?,?,?,?,40,41)")
    .run(id, sequence(db), request, mediaId);
}
function tag(db, id, parent, path, root = "/photos") {
  db.prepare("INSERT INTO system_tags(id,parent_id,source_root,directory_path,name,last_seen_at) VALUES(?,?,?,?,?,30)").run(id,parent,root,path,id);
}

test("v2 defines all required media columns, nullable metadata and zero caches", () => fixture((db) => {
  const columns = new Set(db.prepare("PRAGMA table_info(media)").all().map((r) => r.name));
  for (const name of ["id","fingerprint","file_path","file_name","media_type","file_size","created_time","modified_time","imported_at","last_seen_at","width","height","duration","thumbnail_path","like_count","view_count","play_count","last_viewed_at","last_played_at","playback_position","missing"]) assert.ok(columns.has(name), name);
  media(db);
  const row = db.prepare("SELECT * FROM media").get();
  assert.equal(row.like_count + row.view_count + row.play_count, 0);
  assert.equal(row.playback_position, 0);
  assert.equal(row.width, null);
  assert.equal(row.missing, 0);
  assert.equal(db.prepare("SELECT count(*) AS n FROM media_revisions").get().n, 1);
}));

test("duplicate fingerprints preserve independent media; active path and IDs are unique", () => fixture((db) => {
  media(db); media(db,"m2");
  assert.equal(db.prepare("SELECT count(*) AS n FROM media WHERE fingerprint='same-content'").get().n, 2);
  assert.throws(() => media(db,"m3","/m1.jpg"), /UNIQUE/);
  assert.throws(() => media(db,"m1","/another.jpg"), /UNIQUE/);
}));

test("one media has multiple custom tags and one tag has multiple media", () => fixture((db) => {
  media(db); media(db,"m2");
  db.exec("INSERT INTO custom_tags VALUES('a','Travel',1,1),('b','Family',1,1); INSERT INTO media_custom_tags VALUES('m1','a'),('m1','b'),('m2','a')");
  assert.equal(db.prepare("SELECT count(*) AS n FROM media_custom_tags WHERE media_id='m1'").get().n, 2);
  assert.equal(db.prepare("SELECT count(*) AS n FROM media_custom_tags WHERE tag_id='a'").get().n, 2);
  assert.throws(() => db.exec("INSERT INTO media_custom_tags VALUES('m1','a')"), /UNIQUE/);
}));

test("custom tags are flat, name uniqueness is ASCII case insensitive, rename retains links", () => fixture((db) => {
  media(db);
  db.exec("INSERT INTO custom_tags VALUES('a','Travel',1,1); INSERT INTO media_custom_tags VALUES('m1','a')");
  assert.ok(!db.prepare("PRAGMA table_info(custom_tags)").all().some((r) => r.name === "parent_id"));
  assert.throws(() => db.exec("INSERT INTO custom_tags VALUES('b','travel',1,1)"), /UNIQUE/);
  assert.throws(() => db.exec("INSERT INTO custom_tags VALUES('b','  ',1,1)"), /CHECK/);
  db.exec("UPDATE custom_tags SET name='Trips',updated_at=2 WHERE id='a'");
  assert.equal(db.prepare("SELECT tag_id FROM media_custom_tags").get().tag_id, "a");
  assert.equal(db.prepare("SELECT file_path FROM media").get().file_path, "/m1.jpg");
}));

test("custom tag delete cascades only associations, never media", () => fixture((db) => {
  media(db); media(db,"m2");
  db.exec("INSERT INTO custom_tags VALUES('a','Travel',1,1); INSERT INTO media_custom_tags VALUES('m1','a'),('m2','a'); DELETE FROM custom_tags WHERE id='a'");
  assert.equal(db.prepare("SELECT count(*) AS n FROM media_custom_tags").get().n, 0);
  assert.equal(db.prepare("SELECT count(*) AS n FROM media").get().n, 2);
}));

test("missing foreign keys fail and failed association replacement rolls back", () => fixture((db) => {
  media(db);
  db.exec("INSERT INTO custom_tags VALUES('a','Travel',1,1); INSERT INTO media_custom_tags VALUES('m1','a')");
  for (const sql of ["INSERT INTO media_custom_tags VALUES('absent','a')", "INSERT INTO media_custom_tags VALUES('m1','absent')", "INSERT INTO media_system_tags VALUES('m1','absent')"]) assert.throws(() => db.exec(sql), /FOREIGN KEY/);
  assert.throws(() => like(db, "bad-like", "absent"), /FOREIGN KEY/);
  db.exec("BEGIN; DELETE FROM media_custom_tags WHERE media_id='m1'");
  assert.throws(() => db.exec("INSERT INTO media_custom_tags VALUES('m1','absent')"), /FOREIGN KEY/);
  db.exec("ROLLBACK");
  assert.equal(db.prepare("SELECT tag_id FROM media_custom_tags").get().tag_id, "a");
}));

test("system directory tree supports deep levels and rejects cycles/cross-root/orphans", () => fixture((db) => {
  tag(db,"root",null,"/photos");
  let parent = "root";
  let directory = "/photos";
  for (let n=0; n<128; n++) { directory += `/${n}`; tag(db,`d${n}`,parent,directory); parent=`d${n}`; }
  assert.equal(db.prepare("WITH RECURSIVE tree(id) AS (SELECT 'root' UNION ALL SELECT t.id FROM system_tags t JOIN tree p ON t.parent_id=p.id) SELECT count(*) AS n FROM tree").get().n, 129);
  assert.throws(() => db.prepare("UPDATE system_tags SET parent_id=? WHERE id='root'").run(parent), /cycle/);
  assert.throws(() => tag(db,"self","self","/photos/self"), /CHECK/);
  assert.throws(() => tag(db,"orphan","missing","/photos/orphan"), /FOREIGN KEY/);
  tag(db,"other",null,"/other","/other");
  assert.throws(() => tag(db,"cross","other","/photos/cross"), /FOREIGN KEY/);
  assert.throws(() => tag(db,"duplicate",null,"/photos"), /UNIQUE/);
  assert.throws(() => db.exec("INSERT INTO system_tags(id,source_root,directory_path,name,source,last_seen_at) VALUES('fake','/fake','/fake','fake','manual',1)"), /CHECK/);
}));

test("system parent deletion is restricted; leaf deletion cascades links", () => fixture((db) => {
  media(db); tag(db,"root",null,"/photos"); tag(db,"leaf","root","/photos/leaf");
  db.exec("INSERT INTO media_system_tags VALUES('m1','leaf')");
  assert.throws(() => db.exec("DELETE FROM system_tags WHERE id='root'"), /FOREIGN KEY/);
  db.exec("DELETE FROM system_tags WHERE id='leaf'");
  assert.equal(db.prepare("SELECT count(*) AS n FROM media_system_tags").get().n, 0);
  assert.equal(db.prepare("SELECT count(*) AS n FROM media").get().n, 1);
}));

test("unreferenced media deletion cascades owned data but preserves custom tags", () => fixture((db) => {
  media(db); db.exec("INSERT INTO custom_tags VALUES('a','Travel',1,1); INSERT INTO media_custom_tags VALUES('m1','a'); DELETE FROM media WHERE id='m1'");
  for (const table of ["media_custom_tags","media_revisions","media_locations"]) assert.equal(db.prepare(`SELECT count(*) AS n FROM ${table}`).get().n, 0);
  assert.equal(db.prepare("SELECT count(*) AS n FROM custom_tags").get().n, 1);
}));

test("location updates retain identity/import date/tags/events and record history", () => fixture((db) => {
  media(db); like(db);
  db.exec("INSERT INTO custom_tags VALUES('a','Travel',1,1); INSERT INTO media_custom_tags VALUES('m1','a'); UPDATE media SET file_path='/renamed.jpg',file_name='renamed.jpg',last_seen_at=50 WHERE id='m1'");
  const row = db.prepare("SELECT * FROM media").get();
  assert.equal(row.id,"m1"); assert.equal(row.imported_at,20); assert.equal(row.like_count,1);
  assert.equal(db.prepare("SELECT count(*) AS n FROM media_locations").get().n,2);
  assert.equal(db.prepare("SELECT tag_id FROM media_custom_tags").get().tag_id,"a");
  assert.throws(() => db.exec("UPDATE media SET id='changed'"), /immutable/);
  assert.throws(() => db.exec("UPDATE media SET imported_at=99"), /immutable/);
}));

test("same-path replacement can supersede old identity without history inheritance", () => fixture((db) => {
  media(db); like(db);
  db.exec("UPDATE media SET state='superseded',missing=1 WHERE id='m1'");
  media(db,"replacement","/m1.jpg","new-content");
  const row = db.prepare("SELECT * FROM media WHERE id='replacement'").get();
  assert.equal(row.like_count,0); assert.equal(row.playback_position,0);
  assert.equal(db.prepare("SELECT media_id FROM like_events").get().media_id,"m1");
  assert.throws(() => db.exec("DELETE FROM media WHERE id='m1'"), /FOREIGN KEY/);
}));

test("like insertion and cache roll back together; repeated active likes remain distinct", () => fixture((db) => {
  media(db);
  db.exec("BEGIN"); like(db); assert.equal(db.prepare("SELECT like_count FROM media").get().like_count,1); db.exec("ROLLBACK");
  assert.equal(db.prepare("SELECT count(*) AS n FROM like_events").get().n,0);
  assert.equal(db.prepare("SELECT like_count FROM media").get().like_count,0);
  for (let n=0; n<5; n++) like(db,`l${n}`);
  assert.equal(db.prepare("SELECT like_count FROM media").get().like_count,5);
  assert.throws(() => like(db,"duplicate","m1","l0"), /UNIQUE/);
  assert.equal(db.prepare("SELECT like_count FROM media").get().like_count,5);
  assert.throws(() => db.exec("UPDATE like_events SET occurred_at=100"), /immutable/);
  assert.throws(() => db.exec("DELETE FROM like_events"), /immutable/);
}));

test("counter overflow aborts event insertion rather than losing atomicity", () => fixture((db) => {
  media(db); db.exec("UPDATE media SET like_count=9223372036854775807 WHERE id='m1'");
  assert.throws(() => like(db), /INTEGER|CHECK/);
  assert.equal(db.prepare("SELECT count(*) AS n FROM like_events").get().n,0);
}));

test("activation uniqueness, last-view timestamp, and immutable view source", () => fixture((db) => {
  media(db);
  const insert = db.prepare("INSERT INTO view_events(id,ingest_seq,request_id,media_id,activation_id,occurred_at,recorded_at) VALUES(?,?,?,'m1',?,?,100)");
  insert.run("v1",sequence(db),"r1","a1",90); insert.run("v2",sequence(db),"r2","a2",50);
  assert.equal(db.prepare("SELECT view_count FROM media").get().view_count,2);
  assert.equal(db.prepare("SELECT last_viewed_at FROM media").get().last_viewed_at,90);
  assert.throws(() => insert.run("v3",sequence(db),"r3","a1",100), /UNIQUE/);
  assert.throws(() => db.exec("UPDATE view_events SET activation_id='other'"), /immutable/);
}));

test("qualified play data references content version and only finalizes once without recounting", () => fixture((db) => {
  media(db);
  const insert = db.prepare("INSERT INTO play_events(id,ingest_seq,request_id,media_id,content_version_id,player_session_id,started_at,qualified_at,start_position,end_position,watched_seconds,completed) VALUES(?,?,?,'m1',?,?,100,5100,0,5,5,0)");
  assert.throws(() => insert.run("bad",sequence(db),"bad",99,"bad"), /FOREIGN KEY/);
  insert.run("p1",sequence(db),"r1",1,"player1");
  assert.throws(() => insert.run("p2",sequence(db),"r2",1,"player1"), /UNIQUE/);
  assert.throws(() => db.exec("UPDATE play_events SET media_id='other',ended_at=6000"), /finalization/);
  db.exec("UPDATE play_events SET ended_at=6000,end_position=6,watched_seconds=6,completed=1,end_reason='closed' WHERE id='p1'");
  assert.equal(db.prepare("SELECT play_count FROM media").get().play_count,1);
  assert.equal(db.prepare("SELECT last_played_at FROM media").get().last_played_at,5100);
  assert.throws(() => db.exec("UPDATE play_events SET ended_at=7000"), /finalization/);
  assert.throws(() => db.exec("DELETE FROM play_events"), /immutable/);
}));

test("independent playback state updates cache but never creates play events", () => fixture((db) => {
  media(db); db.exec("INSERT INTO playback_states(media_id,content_version_id,position_seconds,duration_seconds,completed,updated_at) VALUES('m1',1,8,10,0,100)");
  assert.equal(db.prepare("SELECT playback_position FROM media").get().playback_position,8);
  db.exec("UPDATE playback_states SET position_seconds=9 WHERE media_id='m1'");
  assert.equal(db.prepare("SELECT playback_position FROM media").get().playback_position,9);
  assert.equal(db.prepare("SELECT play_count FROM media").get().play_count,0);
  assert.equal(db.prepare("SELECT count(*) AS n FROM play_events").get().n,0);
  assert.throws(() => db.exec("INSERT INTO playback_states VALUES('m1',99,0,10,0,1,1)"), /FOREIGN KEY/);
  db.exec("DELETE FROM playback_states");
  assert.equal(db.prepare("SELECT playback_position FROM media").get().playback_position,0);
}));

test("quick snapshots require seed, preserve ordinal, enforce uniqueness and cascade on session deletion", () => fixture((db) => {
  media(db); media(db,"m2");
  assert.throws(() => db.exec("INSERT INTO quick_browse_sessions(id,sort_mode,created_at) VALUES('bad','random',1)"), /CHECK/);
  db.exec("INSERT INTO quick_browse_sessions(id,sort_mode,seed,created_at) VALUES('s1','random','stable',1); INSERT INTO quick_browse_session_items VALUES('s1',0,'m2',1),('s1',1,'m1',1)");
  assert.throws(() => db.exec("INSERT INTO quick_browse_session_items VALUES('s1',2,'m1',1)"), /UNIQUE/);
  db.exec("UPDATE quick_browse_sessions SET item_count=2,state='ready' WHERE id='s1'");
  assert.deepEqual(db.prepare("SELECT media_id FROM quick_browse_session_items WHERE session_id='s1' ORDER BY ordinal").all().map((r)=>r.media_id),["m2","m1"]);
  assert.throws(() => db.exec("UPDATE quick_browse_sessions SET seed='changed'"), /immutable/);
  assert.throws(() => db.exec("DELETE FROM quick_browse_session_items WHERE ordinal=0"), /immutable/);
  assert.throws(() => db.exec("DELETE FROM media WHERE id='m1'"), /FOREIGN KEY/);
  db.exec("DELETE FROM quick_browse_sessions WHERE id='s1'");
  assert.equal(db.prepare("SELECT count(*) AS n FROM quick_browse_session_items").get().n,0);
}));

test("quick snapshot publish rejects gaps and incomplete counts", () => fixture((db) => {
  media(db); db.exec("INSERT INTO quick_browse_sessions(id,sort_mode,created_at) VALUES('s','imported',1); INSERT INTO quick_browse_session_items VALUES('s',2,'m1',1)");
  assert.throws(() => db.exec("UPDATE quick_browse_sessions SET item_count=1,state='ready'"), /contiguous/);
  assert.equal(db.prepare("SELECT state FROM quick_browse_sessions").get().state,"building");
}));

test("settings keep structured JSON values and reject invalid JSON/duplicate keys", () => fixture((db) => {
  db.prepare("INSERT INTO app_settings VALUES('layout',?,1)").run(JSON.stringify({columns:4,filters:["image","video"]}));
  assert.deepEqual(JSON.parse(db.prepare("SELECT value_json FROM app_settings").get().value_json),{columns:4,filters:["image","video"]});
  assert.throws(() => db.exec("INSERT INTO app_settings VALUES('bad','not-json',1)"), /CHECK/);
  assert.throws(() => db.exec("INSERT INTO app_settings VALUES('layout','{}',1)"), /UNIQUE/);
}));

test("storage invariants also work with SQLite default recursive_triggers disabled", () => fixture((db) => {
  db.exec("PRAGMA recursive_triggers=OFF"); media(db); like(db);
  db.exec("INSERT INTO playback_states(media_id,content_version_id,position_seconds,duration_seconds,completed,updated_at) VALUES('m1',1,4,10,0,1)");
  db.exec("UPDATE media SET file_path='/moved.jpg',file_name='moved.jpg' WHERE id='m1'");
  assert.equal(db.prepare("SELECT like_count FROM media").get().like_count,1);
  assert.equal(db.prepare("SELECT playback_position FROM media").get().playback_position,4);
  assert.equal(db.prepare("SELECT count(*) AS n FROM media_locations").get().n,2);
}));

test("new content versions isolate playback state and reject state identity reassignment", () => fixture((db) => {
  media(db); media(db,"m2");
  db.exec("INSERT INTO playback_states(media_id,content_version_id,position_seconds,duration_seconds,completed,updated_at) VALUES('m1',1,8,10,0,1)");
  assert.throws(() => db.exec("UPDATE playback_states SET media_id='m2'"), /immutable/);
  db.exec("INSERT INTO media_revisions VALUES('m1',2,'different',64,2); UPDATE media SET current_content_version_id=2 WHERE id='m1'");
  assert.equal(db.prepare("SELECT playback_position FROM media WHERE id='m1'").get().playback_position,0);
  assert.equal(db.prepare("SELECT position_seconds FROM playback_states WHERE media_id='m1' AND content_version_id=1").get().position_seconds,8);
}));

test("invalid numeric, identity and missing-state values cannot enter the model", () => fixture((db) => {
  media(db);
  for (const sql of [
    "UPDATE media SET file_size=-1", "UPDATE media SET width=0", "UPDATE media SET media_type='audio'",
    "UPDATE media SET like_count=-1", "UPDATE media SET missing=1", "UPDATE media SET state='missing'",
    "UPDATE media SET current_content_version_id=99",
  ]) assert.throws(() => db.exec(sql), /CHECK|FOREIGN KEY/);
  assert.throws(() => like(db,"","m1","request"), /CHECK/);
}));

test("INSERT OR REPLACE cannot rewrite identity or logs with recursive triggers off", () => fixture((db) => {
  db.exec("PRAGMA recursive_triggers=OFF"); media(db); like(db);
  assert.throws(() => db.exec("INSERT OR REPLACE INTO like_events SELECT * FROM like_events WHERE id='l1'"), /UNIQUE/);
  db.exec("INSERT INTO view_events(id,ingest_seq,request_id,media_id,activation_id,occurred_at,recorded_at) VALUES('v',100,'vr','m1','a',1,1)");
  assert.throws(() => db.exec("INSERT OR REPLACE INTO view_events SELECT * FROM view_events WHERE id='v'"), /UNIQUE/);
  db.exec("INSERT INTO play_events(id,ingest_seq,request_id,media_id,content_version_id,player_session_id,started_at,qualified_at,start_position,end_position,watched_seconds,completed) VALUES('p',101,'pr','m1',1,'ps',0,5000,0,5,5,0)");
  assert.throws(() => db.exec("INSERT OR REPLACE INTO play_events SELECT * FROM play_events WHERE id='p'"), /UNIQUE/);
  assert.throws(() => db.exec("INSERT OR REPLACE INTO media SELECT * FROM media WHERE id='m1'"), /UNIQUE/);
  const row = db.prepare("SELECT like_count,view_count,play_count FROM media").get();
  assert.deepEqual([row.like_count,row.view_count,row.play_count],[1,1,1]);
}));

test("migration v1 data survives v2; failing upgrade rolls back all new schema", () => {
  const db = new DatabaseSync(":memory:");
  try {
    db.exec(first); db.exec("INSERT INTO app_metadata VALUES('retained','yes'); BEGIN");
    assert.throws(() => db.exec(model + "\nINSERT INTO missing_table VALUES(1);"));
    db.exec("ROLLBACK");
    assert.equal(db.prepare("SELECT count(*) AS n FROM sqlite_schema WHERE name='media'").get().n,0);
    assert.equal(db.prepare("SELECT value FROM app_metadata WHERE key='retained'").get().value,"yes");
    db.exec("PRAGMA foreign_keys=ON; BEGIN"); db.exec(model); db.exec("COMMIT");
    assert.equal(db.prepare("SELECT value FROM app_metadata WHERE key='retained'").get().value,"yes");
    assert.deepEqual(db.prepare("PRAGMA foreign_key_check").all(),[]);
  } finally { db.close(); }
});
