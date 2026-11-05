-- Sample SQL file for snapshot testing

CREATE TABLE users (id SERIAL PRIMARY KEY, email TEXT NOT NULL);
CREATE INDEX idx_users_email ON users(email);
CREATE TABLE orders (id SERIAL PRIMARY KEY, user_id INTEGER NOT NULL);
CREATE VIEW active_orders AS SELECT * FROM orders WHERE status = 'active';
