-- Your SQL goes here
CREATE TABLE IF NOT EXISTS tagmappings (
  id VARCHAR(60) NOT NULL PRIMARY KEY,
  tag VARCHAR(60) NOT NULL,
  product VARCHAR(60) NOT NULL,
  FOREIGN KEY (product) REFERENCES products(id),
  FOREIGN KEY (tag) REFERENCES tags(id)
);
