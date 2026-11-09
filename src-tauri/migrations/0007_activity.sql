-- Preserve existing logs/triggers. Legacy Views have no recorded origin.
ALTER TABLE view_events ADD COLUMN source TEXT NOT NULL DEFAULT 'normal_viewer'
 CHECK(source IN ('grid','normal_viewer','quick_browser','search','history'));
ALTER TABLE view_events ADD COLUMN viewed_at INTEGER GENERATED ALWAYS AS (occurred_at) VIRTUAL;
CREATE TABLE viewer_activations (
 id TEXT PRIMARY KEY, viewer_id TEXT NOT NULL, ordinal INTEGER NOT NULL CHECK(ordinal>0),
 media_id TEXT NOT NULL REFERENCES media(id), content_version_id INTEGER NOT NULL,
 source TEXT NOT NULL, closed INTEGER NOT NULL DEFAULT 0 CHECK(closed IN(0,1)),
 UNIQUE(viewer_id,ordinal),
 FOREIGN KEY(media_id,content_version_id) REFERENCES media_revisions(media_id,content_version_id)
) STRICT;
CREATE TABLE player_sessions (
 id TEXT PRIMARY KEY, activation_id TEXT NOT NULL UNIQUE REFERENCES viewer_activations(id),
 media_id TEXT NOT NULL, content_version_id INTEGER NOT NULL,
 sequence INTEGER NOT NULL DEFAULT 0, payload TEXT NOT NULL DEFAULT '',
 clock_ms REAL NOT NULL DEFAULT 0, received_at INTEGER NOT NULL,
 position REAL NOT NULL DEFAULT 0, duration REAL, rate REAL NOT NULL DEFAULT 1,
 playing INTEGER NOT NULL DEFAULT 0, watched REAL NOT NULL DEFAULT 0,
 covered REAL NOT NULL DEFAULT 0, started_at INTEGER, start_position REAL NOT NULL DEFAULT 0,
 ended_at INTEGER, end_reason TEXT, tail_position REAL,
 FOREIGN KEY(media_id,content_version_id) REFERENCES media_revisions(media_id,content_version_id)
) STRICT;
CREATE INDEX player_open ON player_sessions(id) WHERE ended_at IS NULL;
CREATE TABLE statistics_jobs (
 id TEXT PRIMARY KEY, kind TEXT NOT NULL CHECK(kind IN('all','like','view','play')),
 media_id TEXT REFERENCES media(id), high_water INTEGER NOT NULL,
 state TEXT NOT NULL DEFAULT 'running', phase INTEGER NOT NULL DEFAULT 0,
 scan_cursor INTEGER NOT NULL DEFAULT 0, publish_cursor TEXT NOT NULL DEFAULT '', error TEXT
) STRICT;
CREATE UNIQUE INDEX statistics_one_active ON statistics_jobs((1)) WHERE state IN('running','cleaning');
CREATE TABLE statistics_base (
 job_id TEXT NOT NULL REFERENCES statistics_jobs(id), media_id TEXT NOT NULL REFERENCES media(id),
 likes INTEGER NOT NULL DEFAULT 0, views INTEGER NOT NULL DEFAULT 0, plays INTEGER NOT NULL DEFAULT 0,
 last_view INTEGER, last_play INTEGER, PRIMARY KEY(job_id,media_id)
) STRICT;
CREATE TABLE statistics_delta (
 job_id TEXT NOT NULL REFERENCES statistics_jobs(id), media_id TEXT NOT NULL REFERENCES media(id),
 likes INTEGER NOT NULL DEFAULT 0, views INTEGER NOT NULL DEFAULT 0, plays INTEGER NOT NULL DEFAULT 0,
 last_view INTEGER, last_play INTEGER, PRIMARY KEY(job_id,media_id)
) STRICT;
CREATE TRIGGER statistics_like AFTER INSERT ON like_events BEGIN
 INSERT INTO statistics_delta(job_id,media_id,likes)
 SELECT id,NEW.media_id,1 FROM statistics_jobs WHERE state='running'
 ON CONFLICT(job_id,media_id) DO UPDATE SET likes=likes+1;
END;
CREATE TRIGGER statistics_view AFTER INSERT ON view_events BEGIN
 INSERT INTO statistics_delta(job_id,media_id,views,last_view)
 SELECT id,NEW.media_id,1,NEW.occurred_at FROM statistics_jobs WHERE state='running'
 ON CONFLICT(job_id,media_id) DO UPDATE SET views=views+1,last_view=max(coalesce(last_view,excluded.last_view),excluded.last_view);
END;
CREATE TRIGGER statistics_play AFTER INSERT ON play_events BEGIN
 INSERT INTO statistics_delta(job_id,media_id,plays,last_play)
 SELECT id,NEW.media_id,1,NEW.qualified_at FROM statistics_jobs WHERE state='running'
 ON CONFLICT(job_id,media_id) DO UPDATE SET plays=plays+1,last_play=max(coalesce(last_play,excluded.last_play),excluded.last_play);
END;
