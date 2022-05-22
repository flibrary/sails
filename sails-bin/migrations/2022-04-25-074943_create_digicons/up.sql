-- Your SQL goes here
CREATE TABLE IF NOT EXISTS digicons (
  id VARCHAR(60) NOT NULL PRIMARY KEY,
  creator_id CHAR(36) NOT NULL,
  name VARCHAR(300) NOT NULL,
  link VARCHAR(400) NOT NULL,
  FOREIGN KEY (creator_id) REFERENCES users(id)
);
