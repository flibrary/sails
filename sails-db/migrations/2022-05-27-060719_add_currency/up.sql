-- Your SQL goes here
ALTER TABLE transactions ADD COLUMN currency TEXT NOT NULL DEFAULT "cny";
ALTER TABLE products ADD COLUMN currency TEXT NOT NULL DEFAULT "cny";
