use std::path::PathBuf;

#[cfg(feature = "release-bundle")]
use directories::ProjectDirs;
#[cfg(feature = "release-bundle")]
fn project_dir() -> ProjectDirs {
    ProjectDirs::from("com", "Ramin Kaviani", "Lost In Time").unwrap()
}

#[cfg(feature = "release-bundle")]
fn game_dir() -> PathBuf {
    project_dir().data_dir().to_owned()
}

#[cfg(not(feature = "release-bundle"))]
fn game_dir() -> PathBuf {
    let mut b = PathBuf::new();
    b.push("dev_data");
    b
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
    path.push("save_state.json");
    path
}

pub fn game_data() -> PathBuf {
    let mut path = game_dir();
    path.push("game_data.json");
    path
}
