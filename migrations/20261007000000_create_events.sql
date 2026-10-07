CREATE TABLE events (
    id INTEGER PRIMARY KEY,
    role_id INTEGER NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    occurs_at TEXT NOT NULL,
    notes TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX events_timeline ON events(role_id, occurs_at, id);
CREATE TABLE event_people (
    id INTEGER PRIMARY KEY,
    event_id INTEGER NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    job_title TEXT NOT NULL DEFAULT '',
    email TEXT NOT NULL DEFAULT ''
);
CREATE INDEX event_people_event ON event_people(event_id);
INSERT INTO events (role_id, title, occurs_at) SELECT id, 'Applied', created_at FROM roles;
CREATE TRIGGER roles_applied_event AFTER INSERT ON roles BEGIN
    INSERT INTO events (role_id, title, occurs_at) VALUES (NEW.id, 'Applied', NEW.created_at);
END;
