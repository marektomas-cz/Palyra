use std::{env, path::PathBuf};

pub fn palyra_bin() -> PathBuf {
    if let Ok(path) = env::var("PALYRA_BIN_UNDER_TEST") {
        return PathBuf::from(path);
    }

    PathBuf::from(env!("CARGO_BIN_EXE_palyra"))
}
