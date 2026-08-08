-- Slack notifications were removed from the app; drop the schema that
-- existed only to serve them.
DROP TABLE notification_log;
ALTER TABLE shows DROP COLUMN notify_new_episodes;
