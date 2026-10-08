CREATE TABLE people (
    id INTEGER PRIMARY KEY,
    role_id INTEGER NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    first_name TEXT NOT NULL,
    last_name TEXT NOT NULL DEFAULT '',
    email TEXT NOT NULL DEFAULT '',
    phone TEXT NOT NULL DEFAULT '',
    notes TEXT NOT NULL DEFAULT '',
    UNIQUE(id, role_id)
);
CREATE INDEX people_application ON people(role_id);
CREATE UNIQUE INDEX events_id_application ON events(id, role_id);
ALTER TABLE event_people RENAME TO legacy_event_people;
CREATE TABLE event_people (
    event_id INTEGER NOT NULL,
    person_id INTEGER NOT NULL,
    role_id INTEGER NOT NULL,
    PRIMARY KEY(event_id, person_id),
    FOREIGN KEY(event_id, role_id) REFERENCES events(id, role_id) ON DELETE CASCADE,
    FOREIGN KEY(person_id, role_id) REFERENCES people(id, role_id) ON DELETE CASCADE
);
-- Merge only identical contact details within the same application.
INSERT INTO people (id, role_id, first_name, last_name, email, phone, notes)
SELECT MIN(ep.id), e.role_id, ep.first_name, ep.last_name, ep.email, ep.phone, ep.notes
FROM legacy_event_people ep JOIN events e ON e.id = ep.event_id
GROUP BY e.role_id, ep.first_name, ep.last_name, ep.email, ep.phone, ep.notes;
INSERT INTO event_people (event_id, person_id, role_id)
SELECT DISTINCT ep.event_id, p.id, e.role_id
FROM legacy_event_people ep JOIN events e ON e.id = ep.event_id
JOIN people p ON p.role_id = e.role_id AND p.first_name = ep.first_name
AND p.last_name = ep.last_name AND p.email = ep.email AND p.phone = ep.phone AND p.notes = ep.notes;
DROP TABLE legacy_event_people;
