-- Your SQL goes here
CREATE TABLE IF NOT EXISTS categories (
  id CHAR(36) NOT NULL PRIMARY KEY,
  name VARCHAR(60) NOT NULL,
  priority UNSIGNED BIG INT NOT NULL,
  parent_id CHAR(36),
  is_leaf BOOLEAN NOT NULL,
  FOREIGN KEY (parent_id) REFERENCES categories(id)
);
