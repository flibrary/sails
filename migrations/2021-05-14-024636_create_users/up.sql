-- Your SQL goes here
CREATE TABLE IF NOT EXISTS users (
  id CHAR(36) NOT NULL PRIMARY KEY,
  email VARCHAR(60),
  school VARCHAR(100) NOT NULL,
  phone VARCHAR(20) NOT NULL,
  hashed_passwd VARCHAR(60) NOT NULL
);
