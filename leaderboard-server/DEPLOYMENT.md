# Deploying to Shuttle.rs

This guide walks you through deploying the leaderboard server to Shuttle.rs.

## Prerequisites

1. **Install Shuttle CLI**:
   ```bash
   cargo install cargo-shuttle
   ```

2. **Create Shuttle Account**:
   - Visit https://shuttle.rs
   - Sign up for a free account
   - Note: You'll get a free tier with PostgreSQL database included

## Deployment Steps

### 1. Login to Shuttle

```bash
cd leaderboard-server
cargo shuttle login
```

This will open a browser for authentication.

### 2. Initialize Shuttle Project (First Time Only)

```bash
cargo shuttle project start
```

This creates your project on Shuttle's infrastructure.

### 3. Deploy the Server

```bash
cargo shuttle deploy
```

The deployment will:
- ✅ Build your Rust application
- ✅ Provision a PostgreSQL database automatically
- ✅ Run migrations
- ✅ Start the server on HTTPS
- ✅ Give you a public URL like: `https://lost-in-time-leaderboard.shuttleapp.rs`

### 4. Check Deployment Status

```bash
# View deployment logs
cargo shuttle logs

# Check project status
cargo shuttle project status

# List your deployments
cargo shuttle deployment list
```

## Local Testing Before Deployment

Test your Shuttle-compatible code locally:

```bash
# Run with local Shuttle environment
cargo shuttle run

# This will:
# - Create a local PostgreSQL database (if needed)
# - Run migrations
# - Start the server on http://localhost:8000
```

## Configuration

### Shuttle.toml

Edit `Shuttle.toml` to customize your deployment:

```toml
[build]
# Add build options if needed

[deploy]
# Change this to your desired project name (must be unique)
name = "your-project-name-leaderboard"
```

### Environment Variables (Optional)

If you need environment variables:

```bash
# Set secrets on Shuttle
cargo shuttle secrets set KEY=value

# Example:
cargo shuttle secrets set ADMIN_PASSWORD=secret123
```

## Updating Your Game Client

After deployment, you'll get a URL like:
```
https://lost-in-time-leaderboard.shuttleapp.rs
```

### Update Client Configuration

In `src/client/leaderboard.rs`, change:

```rust
// Before (local development)
const LEADERBOARD_API_URL: &str = "http://localhost:3000/api/leaderboard";

// After (production with fallback)
#[cfg(debug_assertions)]
const LEADERBOARD_API_URL: &str = "http://localhost:3000/api/leaderboard";

#[cfg(not(debug_assertions))]
const LEADERBOARD_API_URL: &str = "https://lost-in-time-leaderboard.shuttleapp.rs/api/leaderboard";
```

This way:
- **Debug builds** (during development) → Use localhost
- **Release builds** (distributed game) → Use production server

### Alternative: Environment Variable

Or use an environment variable for more flexibility:

```rust
lazy_static::lazy_static! {
    static ref LEADERBOARD_API_URL: String = {
        std::env::var("LEADERBOARD_URL")
            .unwrap_or_else(|_| {
                #[cfg(debug_assertions)]
                return "http://localhost:3000/api/leaderboard".to_string();
                
                #[cfg(not(debug_assertions))]
                return "https://lost-in-time-leaderboard.shuttleapp.rs/api/leaderboard".to_string();
            })
    };
}
```

## Managing the Database

### Access Database Directly

```bash
# Get database connection string
cargo shuttle resource list

# Connect with psql
cargo shuttle resource tunnel postgres
# Then in another terminal:
psql <connection-string-from-above>
```

### Run Migrations Manually

```bash
# SSH into your deployment
cargo shuttle project logs

# Or run migrations locally against production DB
DATABASE_URL="shuttle-provided-url" sqlx migrate run
```

## Common Commands

```bash
# Deploy latest changes
cargo shuttle deploy

# View real-time logs
cargo shuttle logs --follow

# Restart the service
cargo shuttle project restart

# Stop the service
cargo shuttle project stop

# Start the service
cargo shuttle project start

# Delete the project (⚠️ WARNING: Deletes database!)
cargo shuttle project delete
```

## Cost & Limits (Free Tier)

Shuttle's free tier includes:
- ✅ **1 project** with PostgreSQL
- ✅ **Unlimited requests** (reasonable use)
- ✅ **HTTPS** automatically configured
- ✅ **Auto-scaling**
- ✅ **Database backups**

For production with multiple games or high traffic, consider upgrading to Pro.

## Monitoring

### Check Server Health

```bash
# Health check endpoint
curl https://lost-in-time-leaderboard.shuttleapp.rs/health

# Should return: {"status":"ok"}
```

### Test API Endpoints

```bash
# Fetch top scores
curl https://lost-in-time-leaderboard.shuttleapp.rs/api/leaderboard/top?limit=5

# Submit a test score
curl -X POST https://lost-in-time-leaderboard.shuttleapp.rs/api/leaderboard/submit \
  -H "Content-Type: application/json" \
  -d '{
    "user_id": "test-id",
    "player_name": "TestPlayer",
    "score": 1000,
    "class": "Warrior",
    "chaos_level": 5,
    "mobs_killed": 50,
    "objs_destroyed": 25
  }'
```

## Troubleshooting

### Build Fails

```bash
# Check build logs
cargo shuttle logs

# Try building locally first
cargo build --release
```

### Database Connection Issues

```bash
# Verify database is provisioned
cargo shuttle resource list

# Check database status
cargo shuttle project status
```

### Migration Errors

If migrations fail, you can connect to the database and check:

```bash
cargo shuttle resource tunnel postgres
# Then check migrations table
psql <url> -c "SELECT * FROM _sqlx_migrations;"
```

### Reset Database (Development Only)

```bash
# Delete and recreate project (⚠️ DELETES ALL DATA)
cargo shuttle project delete
cargo shuttle project start
cargo shuttle deploy
```

## CI/CD with GitHub Actions

You can set up automatic deployments on push:

```yaml
# .github/workflows/deploy-leaderboard.yml
name: Deploy Leaderboard

on:
  push:
    branches: [main]
    paths:
      - 'leaderboard-server/**'

jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: shuttle-hq/deploy-action@main
        with:
          deploy-key: ${{ secrets.SHUTTLE_DEPLOY_KEY }}
          working-directory: leaderboard-server
```

Get your deploy key: `cargo shuttle api-key`

---

For more information, see the [Shuttle.rs documentation](https://docs.shuttle.rs/).


