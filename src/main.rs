extern crate dirs;
extern crate hmac_sha256;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn usage() {
    eprintln!("Usage: krust path-to-script.rs [-- args to script]");
}

fn cachedir() -> Option<PathBuf> {
    match dirs::cache_dir() {
        Some(mut d) => {
            d.push("krust");
            Some(d)
        }
        None => {
            match dirs::home_dir() {
                Some(mut d) => {
                    d.push(".krust");
                    d.push("cache");
                    Some(d)
                }
                None => None,
            }
        }
    }
}

fn digest_path(path: &Path) -> String {
    let os_str = path.as_os_str();

    #[cfg(unix)] {
        use std::os::unix::ffi::OsStrExt;
        return digest(os_str.as_bytes());
    }

    #[cfg(windows)] {
        // !!! untested
        use std::os::windows::ffi::OsStrExt;
        let bytes = os_str
            .encode_wide()
            .flat_map(|wchar| {
                [(wchar >> 8) as u8,
                 wchar as u8].iter()
            })
            .collect::<Vec<u8>>();
        return digest(&bytes);
    }

    #[cfg(not(any(unix, windows)))] {
        return digest(os_str.to_string_lossy().as_bytes());
    }
}

fn digest(stuff: &[u8]) -> String {
    use std::fmt::Write;

    let hash = hmac_sha256::Hash::hash(stuff);
    let mut hex = String::with_capacity(hash.len() * 2);
    for b in &hash {
        write!(hex, "{:02x}", b).unwrap();
    }
    hex
}

fn main() {
    let mut args = env::args().skip(1);
    let script_path = match args.next() {
        None => return usage(),
        Some(file) => fs::canonicalize(&file).expect(&format!("Unable to get absolute path for {}", &file)),
    };
    let cachedir = cachedir().expect("Unable to find a usable cache directory!");
    fs::create_dir_all(&cachedir).expect(&format!("Error while create cache directory {}", cachedir.display()));

    let path_digest = digest_path(&script_path);
    let mut executable = cachedir.clone();
    executable.push(path_digest);
}
