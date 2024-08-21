use std::path::PathBuf;

use directories::ProjectDirs;
pub fn project_dir() -> ProjectDirs {
    ProjectDirs::from("com", "Ramin Kaviani", "Lost In Time").unwrap()
}

pub fn logs_dir() -> PathBuf {
    project_dir().data_dir().to_owned()
}
