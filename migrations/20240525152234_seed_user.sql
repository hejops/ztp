INSERT INTO users (user_id, username, password_hash)
VALUES (
   'ddf8994f-d522-4659-8d02-c1d479057be6',
   'admin',
   -- we can store the hash in plaintext without worry
   '$argon2id$v=19$m=19456,t=2,p=1$x/iP6pHqVLyWY42/unsVNg$FTYquTYkGPsbxp0WkkcRJkODkcNKOLZzycUvtV6kGv8'
);

