-- Create the waitlist table
CREATE TABLE waitlist (
    email TEXT PRIMARY KEY,
    created_timestamp TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    allowed BOOLEAN NOT NULL DEFAULT FALSE,
    allowed_timestamp TIMESTAMP WITH TIME ZONE
);

-- Copy distinct emails from users table to the wait-list and set allowed to true
-- as they're already our users
INSERT INTO waitlist (email, allowed)
SELECT DISTINCT email, TRUE FROM users
WHERE email <> '' AND email IS NOT NULL;
