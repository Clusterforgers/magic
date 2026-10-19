ALTER TABLE import_runs ADD COLUMN input text CHECK (input <> '');

COMMENT ON COLUMN import_runs.input IS 'Identifies the data a run read (the CLI stores file checksums), so the next import of the same input is recognised as done.';
