-- Your SQL goes here
CREATE TABLE IF NOT EXISTS digicons (
  id VARCHAR(60) NOT NULL PRIMARY KEY,
  creator_id CHAR(36) NOT NULL,
  name VARCHAR(300) NOT NULL,
  storage_type TEXT NOT NULL,
  storage_detail VARCHAR(400),
  time_created TIMESTAMP NOT NULL,
  time_modified TIMESTAMP NOT NULL,
  FOREIGN KEY (creator_id) REFERENCES users(id)
);
