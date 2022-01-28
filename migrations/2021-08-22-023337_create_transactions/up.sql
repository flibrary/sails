-- Your SQL goes here
CREATE TABLE IF NOT EXISTS transactions (
  id VARCHAR(60) NOT NULL PRIMARY KEY,
  shortid TEXT NOT NULL,
  seller CHAR(36) NOT NULL,
  product VARCHAR(60) NOT NULL,
  buyer CHAR(36) NOT NULL,
  price UNSIGNED BIG INT NOT NULL,
  quantity UNSIGNED BIG INT NOT NULL,
  time_sent TIMESTAMP NOT NULL,
  transaction_status TEXT CHECK(transaction_status IN ('refunded', 'placed', 'paid', 'finished')) NOT NULL,
  FOREIGN KEY (product) REFERENCES products(id),
  FOREIGN KEY (buyer) REFERENCES users(id)
  FOREIGN KEY (seller) REFERENCES users(id)
);
