-- The boss rework closes every run still open: each gets an `Abandoned` event and the abandoned status. Finished and
-- abandoned runs keep their place on the leaderboard.
INSERT INTO delve_events (delve_id, seq, event)
SELECT d.id, COALESCE(MAX(e.seq) + 1, 0), '"Abandoned"'::jsonb
FROM delves d
LEFT JOIN delve_events e ON e.delve_id = d.id
WHERE d.status NOT IN ('victory', 'defeat', 'abandoned')
GROUP BY d.id;

UPDATE delves SET status = 'abandoned', updated_at = now()
WHERE status NOT IN ('victory', 'defeat', 'abandoned');
