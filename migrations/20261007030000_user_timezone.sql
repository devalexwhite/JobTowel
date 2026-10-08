-- NULL means the browser timezone has not been detected or chosen yet.
ALTER TABLE users ADD COLUMN timezone TEXT;
