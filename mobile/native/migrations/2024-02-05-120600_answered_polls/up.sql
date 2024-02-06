create table answered_polls
(
    id        INTEGER PRIMARY KEY NOT NULL,
    poll_id   INTEGER             NOT NULL,
    timestamp BIGINT              NOT NULL

);

create table ignored_polls
(
    id        INTEGER PRIMARY KEY NOT NULL,
    poll_id   INTEGER             NOT NULL,
    timestamp BIGINT              NOT NULL
);
