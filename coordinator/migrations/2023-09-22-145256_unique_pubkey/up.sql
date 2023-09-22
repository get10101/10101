ALTER TABLE "users"
    ADD CONSTRAINT unique_pubkey UNIQUE (pubkey);

