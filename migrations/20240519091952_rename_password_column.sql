-- we switch from storing plaintext passwords to hashed passwords
ALTER TABLE users RENAME password TO password_hash;

