CREATE TABLE roles (
    id INTEGER PRIMARY KEY,
    role_title TEXT NOT NULL,
    posting_url TEXT NOT NULL DEFAULT '',
    company TEXT NOT NULL,
    role_description TEXT NOT NULL DEFAULT '',
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX roles_user_id ON roles(user_id);
