-- Preserve existing names intact and retain legacy job titles as person notes.
ALTER TABLE event_people RENAME COLUMN name TO first_name;
ALTER TABLE event_people RENAME COLUMN job_title TO notes;
ALTER TABLE event_people ADD COLUMN last_name TEXT NOT NULL DEFAULT '';
ALTER TABLE event_people ADD COLUMN phone TEXT NOT NULL DEFAULT '';
