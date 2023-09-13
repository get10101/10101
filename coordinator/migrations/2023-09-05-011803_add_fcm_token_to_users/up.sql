-- allow storing FCM tokens in users table (for push notifications)
ALTER TABLE
    users
    ADD
        COLUMN fcm_token TEXT NOT NULL DEFAULT '';
