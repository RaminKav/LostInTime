# Shuttle.rs Quick Start

This is a simplified guide to get your leaderboard server deployed to Shuttle.rs in 5 minutes.

## Step 1: Install Shuttle CLI

```bash
cargo install cargo-shuttle
```

## Step 2: Login

```bash
cd leaderboard-server
cargo shuttle login
```

This opens a browser for authentication.

## Step 3: Deploy

```bash
# First deployment (creates project)
cargo shuttle deploy

# That's it! 🚀
```

Shuttle will:
- ✅ Automatically provision a PostgreSQL database
- ✅ Run your migrations
- ✅ Deploy your server with HTTPS
- ✅ Give you a URL like: `https://lost-in-time-leaderboard.shuttleapp.rs`

## Step 4: Update Your Game Client

After deployment, copy the URL Shuttle gives you and update the game client:

**File**: `src/client/leaderboard.rs`

The URL is already configured! Just note your actual deployment URL and update if needed:

```rust
// Current configuration (line 15-20)
#[cfg(debug_assertions)]
const LEADERBOARD_API_URL: &str = "http://localhost:3000/api/leaderboard";

#[cfg(not(debug_assertions))]
const LEADERBOARD_API_URL: &str = "https://YOUR-PROJECT-NAME.shuttleapp.rs/api/leaderboard";
//                                         ^^^^^^^^^^^^^^^^
//                                         Replace with your actual Shuttle URL
```

## Step 5: Test It

```bash
# Check health
curl https://YOUR-PROJECT-NAME.shuttleapp.rs/health

# Fetch leaderboard
curl https://YOUR-PROJECT-NAME.shuttleapp.rs/api/leaderboard/top?limit=5

# Submit test score
curl -X POST https://YOUR-PROJECT-NAME.shuttleapp.rs/api/leaderboard/submit \
  -H "Content-Type: application/json" \
  -d '{
    "user_id": "test-user",
    "player_name": "TestPlayer",
    "score": 1000,
    "class": "Warrior",
    "chaos_level": 5,
    "mobs_killed": 50,
    "objs_destroyed": 25
  }'
```

## Useful Commands

```bash
# View logs
cargo shuttle logs

# Redeploy after changes
cargo shuttle deploy

# Check project status
cargo shuttle project status

# List all your projects
cargo shuttle project list
```

## What Shuttle Does Automatically

1. **Database**: Creates and manages PostgreSQL database (no setup needed!)
2. **Migrations**: Runs `migrations/001_create_leaderboard.sql` automatically
3. **HTTPS**: Provides SSL certificate (your URL is secure by default)
4. **Scaling**: Auto-scales based on traffic
5. **Monitoring**: Built-in logging and status monitoring

## Local Development vs Production

The server code is already set up to work in both environments:

**Local Development:**
```bash
# Run locally with your own PostgreSQL
DATABASE_URL=postgresql://localhost/leaderboard cargo run
```

**Shuttle Deployment:**
```bash
# Shuttle handles everything
cargo shuttle deploy
```

## Customizing Your Deployment

Edit `Shuttle.toml` to change the project name:

```toml
[deploy]
name = "your-custom-name-leaderboard"
```

This will change your URL to: `https://your-custom-name-leaderboard.shuttleapp.rs`

## Cost

- **Free tier**: Perfect for development and small games
- **Pro tier** ($20/mo): For production games with many players
- See: https://shuttle.rs/pricing

## Need Help?

- **Shuttle Docs**: https://docs.shuttle.rs/
- **Shuttle Discord**: https://discord.gg/shuttle
- **Analytics Server Example**: See `../../../analytics-server/bevy-analytics/` for reference

---

**Note**: This server is already configured and ready to deploy! The code in `main.rs` follows the exact same pattern as the analytics server (`bevy-analytics`).


