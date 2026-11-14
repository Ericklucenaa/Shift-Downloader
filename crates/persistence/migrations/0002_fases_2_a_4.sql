-- Fases 2–4: integridade, organização, fila, torrents e mídia.

ALTER TABLE downloads ADD COLUMN kind TEXT NOT NULL DEFAULT 'http';          -- http | torrent | magnet
ALTER TABLE downloads ADD COLUMN category TEXT NOT NULL DEFAULT 'other';     -- video | audio | archive | …
ALTER TABLE downloads ADD COLUMN sha256 TEXT;                                -- calculado ao concluir
ALTER TABLE downloads ADD COLUMN expected_sha256 TEXT;                       -- informado pelo usuário
ALTER TABLE downloads ADD COLUMN speed_limit INTEGER NOT NULL DEFAULT 0;     -- bytes/s, 0 = sem limite
ALTER TABLE downloads ADD COLUMN paused_reason TEXT;                         -- user | schedule
ALTER TABLE downloads ADD COLUMN torrent_meta TEXT;                          -- JSON (arquivos escolhidos, info hash)
ALTER TABLE downloads ADD COLUMN uploaded_bytes INTEGER NOT NULL DEFAULT 0;

CREATE INDEX downloads_sha256 ON downloads(sha256);

CREATE TABLE download_tags (
    download_id TEXT NOT NULL REFERENCES downloads(id) ON DELETE CASCADE,
    tag         TEXT NOT NULL,
    PRIMARY KEY (download_id, tag)
);

-- Vídeo/áudio (yt-dlp) e conversões (FFmpeg): fila e limites próprios.
CREATE TABLE media_jobs (
    id            TEXT PRIMARY KEY,
    kind          TEXT NOT NULL,                 -- download | convert
    source        TEXT NOT NULL,                 -- URL (download) ou caminho (convert)
    title         TEXT NOT NULL,
    output_dir    TEXT NOT NULL,
    output_path   TEXT,                          -- conhecido ao concluir
    options       TEXT NOT NULL,                 -- JSON: formato escolhido / preset
    status        TEXT NOT NULL,                 -- queued | running | paused | completed | failed | cancelled
    progress      REAL NOT NULL DEFAULT 0,       -- 0..1
    total_bytes   INTEGER,
    thumbnail     TEXT,
    duration      REAL,
    error_kind    TEXT,
    error_detail  TEXT,
    created_at    INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL,
    finished_at   INTEGER
);

CREATE INDEX media_jobs_created ON media_jobs(created_at DESC);
