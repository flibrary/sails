-- Your SQL goes here
CREATE TABLE IF NOT EXISTS digiconmappings (
  id VARCHAR(60) NOT NULL PRIMARY KEY,
  digicon VARCHAR(60) NOT NULL,
  product VARCHAR(60) NOT NULL,
  FOREIGN KEY (product) REFERENCES products(id),
  FOREIGN KEY (digicon) REFERENCES digicons(id)
);
