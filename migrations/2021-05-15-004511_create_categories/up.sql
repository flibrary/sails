-- Your SQL goes here
CREATE TABLE IF NOT EXISTS categories (
  id CHAR(36) NOT NULL PRIMARY KEY,
  ctgname VARCHAR(60) NOT NULL,
  parent_id CHAR(36),
  is_leaf BOOLEAN NOT NULL,
  FOREIGN KEY (parent_id) REFERENCES categories(id)
);
