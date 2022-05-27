-- This file should undo anything in `up.sql`
ALTER TABLE transactions DROP COLUMN currency;
ALTER TABLE products DROP COLUMN currency;
