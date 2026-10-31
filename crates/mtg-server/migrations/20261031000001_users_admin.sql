-- The admin account: the only one that makes accounts. The server sets the flag at start from
-- `MTG_ADMIN_USERNAME` with one UPDATE over every row, so at most one account has it and a changed
-- or removed setting demotes the old admin.
ALTER TABLE users ADD COLUMN is_admin boolean NOT NULL DEFAULT false;
