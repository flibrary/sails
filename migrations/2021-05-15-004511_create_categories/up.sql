-- Your SQL goes here
CREATE TABLE IF NOT EXISTS categories (
  id VARCHAR(60) NOT NULL PRIMARY KEY,
  parent_id VARCHAR(60),
  is_leaf BOOLEAN NOT NULL,
  FOREIGN KEY (parent_id) REFERENCES categories(id)
);
