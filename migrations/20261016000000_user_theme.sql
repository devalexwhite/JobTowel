ALTER TABLE users ADD COLUMN theme TEXT NOT NULL DEFAULT 'auto'
    CHECK (theme IN ('auto', 'light', 'dark'));
