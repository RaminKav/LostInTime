# Leaderboard Server

REST API server for the Lost in Time game leaderboard system.

## Tech Stack

- **Axum**: Web framework
- **SQLx**: Async PostgreSQL driver
- **PostgreSQL**: Database

## Setup

### 1. Install PostgreSQL

```bash
# macOS
brew install postgresql@14
brew services start postgresql@14

# Or use Docker
docker run --name postgres -e POSTGRES_PASSWORD=password -p 5432:5432 -d postgres:14
```

### 2. Create Database

```bash
createdb leaderboard
```

### 3. Configure Environment Variables

Create a `.env` file in this directory:

```bash
DATABASE_URL=postgresql://localhost/leaderboard
PORT=3000
RUST_LOG=leaderboard_server=debug,tower_http=debug
```

### 4. Run Migrations

Migrations run automatically on server start, or manually:

```bash
sqlx database create
sqlx migrate run
```

### 5. Run Server

```bash
cargo run
```

Server will start on `http://localhost:3000`

## API Endpoints

### POST `/api/leaderboard/submit`

Submit a score to the leaderboard.

**Request:**

```json
{
  "user_id": "uuid-here",
  "player_name": "YourName",
  "score": 12450,
  "class": "Mage",
  "chaos_level": 8,
  "mobs_killed": 156,
  "objs_destroyed": 89
}
```

**Response:**

```json
{
  "success": true,
  "rank": 3,
  "is_personal_best": true,
  "message": "Score submitted successfully"
}
```

### GET `/api/leaderboard/top?limit=5&class=Mage`

Fetch top scores.

**Query Parameters:**

- `limit` (optional, default 5, max 100): Number of entries to return
- `class` (optional): Filter by class (Warrior, Rogue, Mage, etc.)

**Response:**

```json
{
  "entries": [
    {
      "rank": 1,
      "player_name": "TopPlayer",
      "score": 15000,
      "class": "Rogue",
      "chaos_level": 12,
      "submitted_at": "2025-11-28T10:30:00Z"
    }
  ],
  "last_updated": "2025-11-28T12:00:00Z"
}
```

### GET `/health`

Health check endpoint.

## Development

```bash
# Check code
cargo check

# Run with auto-reload
cargo watch -x run

# Run tests
cargo test
```

## Deployment

### Option 1: Shuttle.rs (Recommended)

Already using Shuttle for analytics, can add this as another service.

```bash
cargo shuttle deploy
```

### Option 2: Fly.io

```bash
fly launch
fly deploy
```

### Option 3: Railway

Connect your GitHub repo, Railway will auto-deploy on push.

## Database Maintenance

For detailed database operations including viewing data, resetting tables, backups, and common queries, see **[DATABASE.md](DATABASE.md)**.

Quick commands:

```sql
-- View top scores
SELECT * FROM leaderboard ORDER BY score DESC LIMIT 10;

-- View user stats
SELECT * FROM users ORDER BY best_score DESC LIMIT 10;

-- Clear test data
DELETE FROM leaderboard WHERE user_id = 'test-user-id';
```

