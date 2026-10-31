-- 0002_add_speaker_to_transcript.sql
-- Add a nullable speaker column to the transcripts table.
-- This column stores the speaker identifier for each segment when
-- the transcript is stored in the DB (e.g., "Speaker_01").
ALTER TABLE transcripts
ADD COLUMN speaker TEXT NULL;