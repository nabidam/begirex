PRAGMA journal_mode = WAL;      -- crash-safe, concurrent read while writing
PRAGMA foreign_keys = ON;

-- One row per download unit. Playlist entries are individual rows.
CREATE TABLE items (
  id                INTEGER PRIMARY KEY AUTOINCREMENT,
  url               TEXT    NOT NULL,
  playlist_id       TEXT,                       -- groups playlist-derived items; NULL for singles
  title             TEXT,                       -- filled after probe/first metadata line
  stage             TEXT    NOT NULL
                    CHECK (stage IN ('queued','downloading','merging',
                                     'completed','paused','error','cancelled')),
  format_expr       TEXT    NOT NULL,           -- the authoritative selector passed to yt-dlp
  output_dir        TEXT    NOT NULL,           -- resolved absolute dir at add-time
  output_template   TEXT    NOT NULL,           -- e.g. %(title)s.%(ext)s
  proxy             TEXT,                       -- effective proxy (override or global snapshot); NULL = none
  extra_args        TEXT,                       -- free-form yt-dlp flags (space-delimited, stored verbatim)
  preset_id         INTEGER REFERENCES presets(id) ON DELETE SET NULL,
  total_bytes       INTEGER,                    -- known after start; NULL until then
  downloaded_bytes  INTEGER NOT NULL DEFAULT 0, -- checkpointed for resume-observability
  percent           REAL    NOT NULL DEFAULT 0, -- 0..100, last checkpoint
  speed_bps         INTEGER,                    -- last observed; NULL when not downloading
  eta_seconds       INTEGER,                    -- last observed; NULL when unknown
  resume_capable    INTEGER NOT NULL DEFAULT 1, -- 0/1; yt-dlp reports whether partial resume is possible
  output_path       TEXT,                       -- final resolved file path once known/completed
  error_message     TEXT,                       -- last stderr summary when stage='error'
  queue_position    INTEGER NOT NULL,           -- ordering among items; drives scheduling + reorder
  created_at        INTEGER NOT NULL,
  updated_at        INTEGER NOT NULL
);
CREATE INDEX idx_items_stage    ON items(stage);           -- sidebar filters + scheduler scan
CREATE INDEX idx_items_position ON items(queue_position);  -- ordered list + next-to-run
CREATE INDEX idx_items_playlist ON items(playlist_id);     -- playlist grouping

-- Named config bundles.
CREATE TABLE presets (
  id               INTEGER PRIMARY KEY AUTOINCREMENT,
  name             TEXT    NOT NULL UNIQUE,
  format_expr      TEXT    NOT NULL,
  output_template  TEXT    NOT NULL,
  proxy            TEXT,                         -- NULL = inherit global
  extra_args       TEXT,
  is_default       INTEGER NOT NULL DEFAULT 0,   -- exactly one row = 1 (enforced in preset_service)
  created_at       INTEGER NOT NULL,
  updated_at       INTEGER NOT NULL
);
CREATE UNIQUE INDEX idx_presets_default ON presets(is_default) WHERE is_default = 1; -- DB-level single-default guard

-- Flat key/value app settings. One row per key.
CREATE TABLE settings (
  key    TEXT PRIMARY KEY,
  value  TEXT
);
-- Seeded keys: global_proxy, default_concurrency (N), default_output_dir,
-- default_output_template, default_preset_id, ytdlp_path, ffmpeg_path,
-- build_flavor, ytdlp_version, ffmpeg_version.

-- Ring-buffered log tail per item (S5 log disclosure). Trimmed to last K lines/item.
CREATE TABLE item_logs (
  id       INTEGER PRIMARY KEY AUTOINCREMENT,
  item_id  INTEGER NOT NULL REFERENCES items(id) ON DELETE CASCADE,
  ts       INTEGER NOT NULL,
  stream   TEXT    NOT NULL CHECK (stream IN ('stdout','stderr')),
  line     TEXT    NOT NULL
);
CREATE INDEX idx_logs_item ON item_logs(item_id, id);
