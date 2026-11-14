-- Fase 1: tarefas de download, linha do tempo de eventos e configurações.
-- O checkpoint de bytes é do aria2 (arquivo .aria2 + sessão); aqui ficam o estado
-- da tarefa e o que é preciso para reconstruí-la no motor depois de uma queda.

CREATE TABLE downloads (
    id               TEXT PRIMARY KEY,           -- UUID v4
    source_url       TEXT NOT NULL,              -- como o usuário colou
    effective_url    TEXT NOT NULL,              -- depois dos redirecionamentos validados
    protocol         TEXT NOT NULL,              -- http | https
    file_name        TEXT NOT NULL,
    directory        TEXT NOT NULL,
    total_bytes      INTEGER,                    -- NULL = tamanho desconhecido
    completed_bytes  INTEGER NOT NULL DEFAULT 0,
    status           TEXT NOT NULL,
    priority         INTEGER NOT NULL DEFAULT 0,
    engine           TEXT NOT NULL,              -- aria2
    engine_ref       TEXT,                       -- GID do aria2 (16 hex), definido por nós
    resumable        INTEGER,                    -- 1/0/NULL (servidor aceita Range?)
    etag             TEXT,
    last_modified    TEXT,
    content_type     TEXT,
    connections      INTEGER NOT NULL DEFAULT 8,
    error_kind       TEXT,
    error_detail     TEXT,
    retry_count      INTEGER NOT NULL DEFAULT 0,
    next_retry_at    INTEGER,
    created_at       INTEGER NOT NULL,           -- ms desde a época Unix
    updated_at       INTEGER NOT NULL,
    started_at       INTEGER,
    finished_at      INTEGER
);

CREATE INDEX downloads_status ON downloads(status);
CREATE INDEX downloads_created ON downloads(created_at DESC);

CREATE TABLE download_events (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    download_id  TEXT NOT NULL REFERENCES downloads(id) ON DELETE CASCADE,
    at           INTEGER NOT NULL,
    kind         TEXT NOT NULL,                  -- created | status | retry | recovered | error
    from_status  TEXT,
    to_status    TEXT,
    detail       TEXT                            -- nunca contém URL completa nem credenciais
);

CREATE INDEX download_events_download ON download_events(download_id, at);

CREATE TABLE settings (
    key    TEXT PRIMARY KEY,
    value  TEXT NOT NULL                         -- JSON
);
