-- Your SQL goes here
CREATE TABLE IF NOT EXISTS products (
  id CHAR(36) NOT NULL PRIMARY KEY,
  shortid TEXT NOT NULL,
  seller_id CHAR(36) NOT NULL,
  operator_id CHAR(36) NOT NULL,
  category CHAR(36) NOT NULL,
  prodname VARCHAR(60) NOT NULL,
  price UNSIGNED BIG INT NOT NULL,
  description VARCHAR(400) NOT NULL,
  product_status TEXT CHECK(product_status IN ('normal', 'sold', 'verified', 'disabled')) NOT NULL,
  FOREIGN KEY (seller_id) REFERENCES users(id),
  FOREIGN KEY (operator_id) REFERENCES users(id),
  FOREIGN KEY (category) REFERENCES categories(id)
);
