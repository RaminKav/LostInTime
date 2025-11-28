-- Create leaderboard table
CREATE TABLE IF NOT EXISTS leaderboard (
    id SERIAL PRIMARY KEY,
    user_id VARCHAR(255) NOT NULL,
    player_name VARCHAR(100) NOT NULL,
    score INTEGER NOT NULL CHECK (score >= 0),
    class VARCHAR(50) NOT NULL,
    chaos_level INTEGER NOT NULL DEFAULT 0,
    mobs_killed INTEGER NOT NULL DEFAULT 0,
    objs_destroyed INTEGER NOT NULL DEFAULT 0,
    submitted_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    -- Prevent score inflation from same user
    UNIQUE(user_id, score, submitted_at)
);

-- Index for fast leaderboard queries (ORDER BY score DESC)
CREATE INDEX idx_leaderboard_score ON leaderboard(score DESC, submitted_at DESC);

-- Index for user-specific queries
CREATE INDEX idx_leaderboard_user ON leaderboard(user_id);

-- Index for class-specific leaderboards
CREATE INDEX idx_leaderboard_class ON leaderboard(class, score DESC);

-- Optional: User profiles table
CREATE TABLE IF NOT EXISTS users (
    user_id VARCHAR(255) PRIMARY KEY,
    last_known_name VARCHAR(100),
    total_runs INTEGER DEFAULT 0,
    best_score INTEGER DEFAULT 0,
    first_seen TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    last_seen TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);


