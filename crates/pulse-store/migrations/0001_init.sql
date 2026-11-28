CREATE TABLE folders (
    id                   TEXT PRIMARY KEY NOT NULL,
    label                TEXT NOT NULL,
    path                 TEXT NOT NULL UNIQUE,
    mode                 TEXT NOT NULL,
    paused               INTEGER NOT NULL DEFAULT 0,
    rescan_interval_secs INTEGER NOT NULL,
    created_at_ms        INTEGER NOT NULL,
    -- Highest change sequence handed out in this folder.
    sequence             INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE files (
    folder_id   TEXT NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
    path        TEXT NOT NULL,
    size        INTEGER NOT NULL,
    mtime_ns    INTEGER NOT NULL,
    hash        TEXT NOT NULL,          -- hex BLAKE3, '' when deleted
    deleted     INTEGER NOT NULL DEFAULT 0,
    version     TEXT NOT NULL,          -- JSON version vector
    sequence    INTEGER NOT NULL,
    chunk_count INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (folder_id, path)
) WITHOUT ROWID;

CREATE INDEX files_by_sequence ON files (folder_id, sequence);

CREATE TABLE chunks (
    folder_id TEXT NOT NULL,
    path      TEXT NOT NULL,
    idx       INTEGER NOT NULL,
    offset    INTEGER NOT NULL,
    len       INTEGER NOT NULL,
    hash      TEXT NOT NULL,
    PRIMARY KEY (folder_id, path, idx),
    FOREIGN KEY (folder_id, path) REFERENCES files (folder_id, path) ON DELETE CASCADE
) WITHOUT ROWID;

-- Lets M2 find an existing local copy of a chunk before downloading it.
CREATE INDEX chunks_by_hash ON chunks (hash);
