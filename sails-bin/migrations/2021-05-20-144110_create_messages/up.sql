-- Your SQL goes here
CREATE TABLE IF NOT EXISTS messages (
  id VARCHAR(60) NOT NULL PRIMARY KEY,
  send CHAR(36) NOT NULL,
  recv CHAR(36) NOT NULL,
  body VARCHAR NOT NULL,
  time_sent TIMESTAMP NOT NULL,
  FOREIGN KEY (send) REFERENCES users(id),
  FOREIGN KEY (recv) REFERENCES users(id)
);
