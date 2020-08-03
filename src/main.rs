extern crate dirs;
extern crate hmac_sha256;

use std::env;
use std::fs;
use std::io;
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

fn preprocess(input: &fs::File, output_base: &Path) -> io::Result<()> {
    use io::{BufRead, Write};

    let mut processed_path = PathBuf::from(output_base);
    processed_path.set_extension("rs");

    // TODO: some sort of locking here to avoid concurrently-running `krust` instances writing to `processed_path`
    let mut processed = fs::File::create(&processed_path)?;

    // TODO: poke rustc folks about source maps or #line macros or such. See rust-lang/rfcs#1573
    let bufreader = io::BufReader::new(input);
    for (number, line) in bufreader.lines().enumerate() {
        let line = line?;
        if number == 0 && line.starts_with("#!/") {
            // assume shebang, so let's drop it
            continue;
        }
        writeln!(processed, "{}", line)?;
    }
    processed.sync_all()
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

    let krustfile = fs::File::open(&script_path).expect(&format!("Unable to open input script {}", script_path.display()));
    preprocess(&krustfile, &executable).unwrap();
}
