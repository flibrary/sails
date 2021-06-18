-- Your SQL goes here
CREATE TABLE IF NOT EXISTS users (
  id VARCHAR(60) NOT NULL PRIMARY KEY COLLATE NOCASE,
  name VARCHAR(30) NOT NULL,
  school VARCHAR(100) NOT NULL,
  hashed_passwd VARCHAR(60) NOT NULL,
  validated BOOLEAN NOT NULL DEFAULT FALSE,
  user_status TEXT CHECK(user_status IN ('normal', 'admin', 'disabled')) NOT NULL
);
