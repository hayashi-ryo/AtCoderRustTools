/// AtCodeのURL情報
pub const BASE_URL: &str = "https://atcoder.jp";

use std::env;
use std::path::PathBuf;
/// セッションファイルの保存先を取得
pub fn get_session_file() -> PathBuf {
    if cfg!(target_os = "windows") {
        dirs::config_dir().unwrap().join("atc/session.json")
    } else if cfg!(target_os = "macos") {
        dirs::data_local_dir().unwrap().join("atc/session.json")
    } else {
        if let Some(xdg_cache) = env::var_os("XDG_CACHE_HOME") {
            return PathBuf::from(xdg_cache).join("atc/session.json");
        }
        PathBuf::from(env::var_os("HOME").unwrap()).join(".cache/atc/session.json")
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use serial_test::serial;

    #[test]
    fn test_get_session_file_default() {
        let session_path = get_session_file();

        if cfg!(target_os = "windows") {
            let expected_path = dirs::config_dir().unwrap().join("atc/session.json");
            assert_eq!(session_path, expected_path);
        } else if cfg!(target_os = "macos") {
            let expected_path = dirs::data_local_dir().unwrap().join("atc/session.json");
            assert_eq!(session_path, expected_path);
        } else {
            // Linux / Unix
            if let Ok(xdg_cache_home) = env::var("XDG_CACHE_HOME") {
                let expected_path = PathBuf::from(xdg_cache_home).join("atc/session.json");
                assert_eq!(session_path, expected_path);
            } else {
                let home = env::var("HOME").expect("HOME 環境変数が設定されていません");
                let expected_path = PathBuf::from(home).join(".cache/atc/session.json");
                assert_eq!(session_path, expected_path);
            }
        }
    }

    #[test]
    #[serial]
    fn test_get_session_file_with_xdg_cache_home() {
        if cfg!(target_os = "linux") {
            let temp_xdg_cache = "/tmp/mock_xdg_cache";
            env::set_var("XDG_CACHE_HOME", temp_xdg_cache);
            let session_path = get_session_file();
            let expected_path = PathBuf::from(temp_xdg_cache).join("atc/session.json");
            assert_eq!(session_path, expected_path);

            // 環境変数をリセット
            env::remove_var("XDG_CACHE_HOME");
        }
    }

    #[test]
    #[serial]
    fn test_get_session_file_without_xdg_cache_home() {
        if cfg!(target_os = "linux") {
            // XDG_CACHE_HOME を一時的に無効化
            let xdg_cache_home = env::var("XDG_CACHE_HOME").ok();
            env::remove_var("XDG_CACHE_HOME");
            let session_path = get_session_file();

            let home = env::var("HOME").expect("HOME 環境変数が設定されていません");
            let expected_path = PathBuf::from(home).join(".cache/atc/session.json");
            assert_eq!(session_path, expected_path);

            // 元の環境変数を復元
            if let Some(original_xdg_cache) = xdg_cache_home {
                env::set_var("XDG_CACHE_HOME", original_xdg_cache);
            }
        }
    }
}
