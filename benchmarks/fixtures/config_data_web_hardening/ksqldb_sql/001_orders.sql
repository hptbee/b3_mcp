CREATE TABLE orders(id int);
CREATE VIEW order_summary AS SELECT o.id FROM orders o JOIN customers c ON c.id=o.customer_id;
INSERT INTO audit_log SELECT id FROM orders;
