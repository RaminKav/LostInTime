use std::path::PathBuf;

use directories::ProjectDirs;
fn project_dir() -> ProjectDirs {
    ProjectDirs::from("com", "Ramin Kaviani", "Lost In Time").unwrap()
}

fn game_dir() -> PathBuf {
    project_dir().data_dir().to_owned()
}

pub fn logs_dir() -> PathBuf {
    let mut path = game_dir();
    path.push("logs");
    path
}

pub fn analytics_dir() -> PathBuf {
    let mut path = game_dir();
    path.push("analytics");
    path
}

pub fn save_file() -> PathBuf {
    let mut path = game_dir();
    path.push("save.json");
    path
}
