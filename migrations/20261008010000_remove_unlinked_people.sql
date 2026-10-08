CREATE INDEX event_people_person ON event_people(person_id);
-- Remove contacts left behind by the previous event-link retention behavior.
DELETE FROM people
WHERE NOT EXISTS (SELECT 1 FROM event_people WHERE person_id = people.id);
