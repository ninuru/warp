CREATE TABLE window_projects (
  id INTEGER PRIMARY KEY NOT NULL,
  window_id INTEGER NOT NULL,
  path TEXT NOT NULL,
  name TEXT NOT NULL,
  active_tab_index INTEGER NOT NULL,
  is_active BOOLEAN NOT NULL,
  is_previous BOOLEAN NOT NULL,
  FOREIGN KEY(window_id) REFERENCES windows(id)
);

ALTER TABLE tabs ADD COLUMN project_path TEXT;
