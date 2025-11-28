# Database Operations Guide

This guide covers common database operations for the Lost in Time leaderboard database.

## Table of Contents

- [Connecting to the Database](#connecting-to-the-database)
- [Viewing Data](#viewing-data)
- [Resetting Data](#resetting-data)
- [Backing Up Data](#backing-up-data)
- [Common Queries](#common-queries)
- [Troubleshooting](#troubleshooting)

---

## Connecting to the Database

### Using psql (PostgreSQL CLI)

```bash
# Connect to the leaderboard database
psql leaderboard

# Or with full connection string
psql postgresql://localhost/leaderboard

# With username (if needed)
psql -U postgres leaderboard
```

### Using a GUI Client

Popular options:

- **pgAdmin**: https://www.pgadmin.org/
- **DBeaver**: https://dbeaver.io/
- **TablePlus**: https://tableplus.com/

Connection details:

- **Host**: localhost
- **Port**: 5432
- **Database**: leaderboard
- **Username**: postgres (or your PostgreSQL username)
- **Password**: (your PostgreSQL password if set)

---

## Viewing Data

### View All Leaderboard Entries

```sql
-- All entries sorted by score
SELECT * FROM leaderboard
ORDER BY score DESC;

-- Top 10 scores
SELECT * FROM leaderboard
ORDER BY score DESC
LIMIT 10;

-- With formatted output
SELECT
    rank,
    player_name,
    score,
    class,
    chaos_level,
    mobs_killed,
    objs_destroyed,
    submitted_at
FROM (
    SELECT
        ROW_NUMBER() OVER (ORDER BY score DESC, submitted_at ASC) as rank,
        *
    FROM leaderboard
) ranked
LIMIT 20;
```

### View User Statistics

```sql
-- All users
SELECT * FROM users
ORDER BY best_score DESC;

-- Detailed user info
SELECT
    user_id,
    last_known_name,
    best_score,
    total_runs,
    last_seen
FROM users
ORDER BY best_score DESC
LIMIT 10;

-- Find a specific user
SELECT * FROM users
WHERE last_known_name ILIKE '%playername%';
```

### Count Records

```sql
-- Total entries in leaderboard
SELECT COUNT(*) as total_entries FROM leaderboard;

-- Total unique users
SELECT COUNT(*) as total_users FROM users;

-- Entries per class
SELECT
    class,
    COUNT(*) as count,
    AVG(score) as avg_score,
    MAX(score) as max_score
FROM leaderboard
GROUP BY class
ORDER BY count DESC;
```

---

## Resetting Data

### Clear All Leaderboard Entries

```sql
-- ⚠️ WARNING: This deletes all scores!
TRUNCATE TABLE leaderboard;

-- Or with CASCADE to reset auto-incrementing IDs
TRUNCATE TABLE leaderboard RESTART IDENTITY CASCADE;
```

### Clear All User Data

```sql
-- ⚠️ WARNING: This deletes all user records!
TRUNCATE TABLE users;

-- Or both tables at once
TRUNCATE TABLE leaderboard, users RESTART IDENTITY CASCADE;
```

### Delete Specific Entries

```sql
-- Delete a specific user's entries
DELETE FROM leaderboard
WHERE user_id = 'user-id-here';

-- Delete low scores (under 100)
DELETE FROM leaderboard
WHERE score < 100;

-- Delete entries from a specific class
DELETE FROM leaderboard
WHERE class = 'TestClass';

-- Delete old entries (older than 30 days)
DELETE FROM leaderboard
WHERE submitted_at < NOW() - INTERVAL '30 days';
```

### Reset Database Completely

```bash
# Drop and recreate the database (outside psql)
dropdb leaderboard
createdb leaderboard

# Then run migrations
cd leaderboard-server
cargo run  # Migrations run automatically on start
```

---

## Backing Up Data

### Export to SQL File

```bash
# Backup entire database
pg_dump leaderboard > backup_$(date +%Y%m%d).sql

# Backup just the data (no schema)
pg_dump --data-only leaderboard > data_backup.sql

# Backup just specific tables
pg_dump -t leaderboard -t users leaderboard > tables_backup.sql
```

### Restore from Backup

```bash
# Restore database from backup
psql leaderboard < backup_20251128.sql

# Or drop and recreate first
dropdb leaderboard
createdb leaderboard
psql leaderboard < backup_20251128.sql
```

### Export to CSV

```sql
-- Export leaderboard to CSV
COPY (
    SELECT * FROM leaderboard
    ORDER BY score DESC
) TO '/tmp/leaderboard_export.csv'
WITH CSV HEADER;

-- Export users to CSV
COPY users
TO '/tmp/users_export.csv'
WITH CSV HEADER;
```

---

## Common Queries

### Find Duplicate Entries

```sql
-- Users with multiple entries
SELECT
    user_id,
    player_name,
    COUNT(*) as entry_count
FROM leaderboard
GROUP BY user_id, player_name
HAVING COUNT(*) > 1
ORDER BY entry_count DESC;
```

### Get Rank for a Specific Score

```sql
-- What rank would a score of 5000 be?
SELECT COUNT(*) + 1 as rank
FROM leaderboard
WHERE score > 5000;
```

### Get Player's Best Score and Rank

```sql
-- Replace 'user-id-here' with actual user ID
SELECT
    player_name,
    score,
    (SELECT COUNT(*) + 1 FROM leaderboard WHERE score > l.score) as rank,
    chaos_level,
    submitted_at
FROM leaderboard l
WHERE user_id = 'user-id-here'
ORDER BY score DESC
LIMIT 1;
```

### Score Distribution

```sql
-- See score ranges
SELECT
    CASE
        WHEN score < 100 THEN '0-99'
        WHEN score < 500 THEN '100-499'
        WHEN score < 1000 THEN '500-999'
        WHEN score < 5000 THEN '1000-4999'
        ELSE '5000+'
    END as score_range,
    COUNT(*) as count
FROM leaderboard
GROUP BY score_range
ORDER BY score_range;
```

### Most Active Players

```sql
SELECT
    last_known_name,
    total_runs,
    best_score,
    last_seen
FROM users
ORDER BY total_runs DESC
LIMIT 10;
```

---

## Troubleshooting

### Check if Database Exists

```bash
psql -l | grep leaderboard
```

### Check Table Structure

```sql
-- View table schema
\d leaderboard
\d users

-- Or more detailed
SELECT column_name, data_type, is_nullable
FROM information_schema.columns
WHERE table_name = 'leaderboard';
```

### Check Database Size

```sql
-- Size of database
SELECT pg_size_pretty(pg_database_size('leaderboard')) as database_size;

-- Size of each table
SELECT
    tablename,
    pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename)) AS size
FROM pg_tables
WHERE schemaname = 'public'
ORDER BY pg_total_relation_size(schemaname||'.'||tablename) DESC;
```

### Check Active Connections

```sql
SELECT
    pid,
    usename,
    application_name,
    client_addr,
    state,
    query
FROM pg_stat_activity
WHERE datname = 'leaderboard';
```

### Kill Stuck Connections

```sql
-- Terminate a specific connection
SELECT pg_terminate_backend(pid)
FROM pg_stat_activity
WHERE datname = 'leaderboard'
  AND pid <> pg_backend_pid();
```

### Rebuild Migrations

If migrations fail or tables are corrupted:

```bash
# Drop everything
psql leaderboard -c "DROP SCHEMA public CASCADE; CREATE SCHEMA public;"

# Recreate tables
psql leaderboard < migrations/001_create_leaderboard.sql

# Or let the server rebuild
cargo run
```

### Test Connection from Server

```bash
# Test database connection
psql $DATABASE_URL -c "SELECT 1;"

# Or from the project directory
source .env
psql $DATABASE_URL -c "SELECT COUNT(*) FROM leaderboard;"
```

---

## Quick Reference

```bash
# Start PostgreSQL (macOS)
brew services start postgresql@14

# Stop PostgreSQL (macOS)
brew services stop postgresql@14

# Connect to database
psql leaderboard

# Run SQL file
psql leaderboard < script.sql

# Common psql commands (once connected)
\dt          # List tables
\d+ table    # Describe table
\q           # Quit
\l           # List databases
\c dbname    # Connect to database
\x           # Toggle expanded display
```

---

## Safety Tips

1. **Always backup before major operations**: Use `pg_dump` before deleting data
2. **Test queries with SELECT first**: Before running DELETE or UPDATE, run a SELECT to see what will be affected
3. **Use transactions for multiple operations**:
   ```sql
   BEGIN;
   DELETE FROM leaderboard WHERE score < 10;
   -- Check results
   SELECT COUNT(*) FROM leaderboard;
   -- If good: COMMIT; if bad: ROLLBACK;
   COMMIT;
   ```
4. **Be careful with TRUNCATE**: Unlike DELETE, TRUNCATE cannot be rolled back in most cases
5. **Keep backups**: Schedule regular backups, especially before updates or maintenance

---

For more information, see the [main README](README.md) or PostgreSQL documentation.
