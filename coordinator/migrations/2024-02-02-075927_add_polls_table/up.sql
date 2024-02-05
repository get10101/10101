-- Your SQL goes here

CREATE TYPE "Poll_Type_Type" AS ENUM ('SingleChoice');

CREATE TABLE polls
(
    id                 SERIAL PRIMARY KEY       NOT NULL,
    poll_type          "Poll_Type_Type"           NOT NULL,
    question           TEXT                     NOT NULL,
    active             BOOLEAN                  NOT NULL,
    creation_timestamp timestamp WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE choices
(
    id      SERIAL PRIMARY KEY NOT NULL,
    poll_id SERIAL REFERENCES polls (id),
    value   TEXT               NOT NULL
);

CREATE TABLE answers
(
    id                 SERIAL PRIMARY KEY       NOT NULL,
    choice_id          SERIAL REFERENCES choices (id),
    trader_pubkey      TEXT                     NOT NULL REFERENCES users (pubkey),
    value              TEXT                     NOT NULL,
    creation_timestamp timestamp WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO polls (poll_type, question, active)
VALUES ('SingleChoice', 'Where did you hear about us?', true);

INSERT INTO choices (poll_id, value)
VALUES (1, 'Social media (X.com, Nostr)'),
       (1, 'Search engine (Google, Duckduckgo)'),
       (1, 'Friends'),
       (1, 'other');
