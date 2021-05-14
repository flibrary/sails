-- Your SQL goes here
CREATE TABLE IF NOT EXISTS products (
  id CHAR(36) NOT NULL PRIMARY KEY,
  seller_id VARCHAR(36) NOT NULL,
  prodname VARCHAR(60) NOT NULL,
  price UNSIGNED BIG INT NOT NULL,
  description VARCHAR(400) NOT NULL,
  FOREIGN KEY (seller_id) REFERENCES users(id)
);
