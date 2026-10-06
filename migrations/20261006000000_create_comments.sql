CREATE TABLE comments (
    id INTEGER PRIMARY KEY,
    body TEXT NOT NULL CHECK (length(trim(body)) BETWEEN 1 AND 2000),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
