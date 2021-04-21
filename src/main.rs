extern crate dirs;
extern crate hmac_sha256;

use std::env;
use std::fmt::Write;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process;

fn usage() {
    eprintln!("Usage:");
    eprintln!("  khaki --show-cache-dir");
    eprintln!("  khaki --clear-cache-dir");
    eprintln!("  khaki [--main] path-to-script.rs [args to script]");
}

fn cachedir() -> Option<PathBuf> {
    match (dirs::cache_dir(), dirs::home_dir()) {
        (Some(mut d), _) => {
            d.push("khaki");
            Some(d)
        }
        (None, Some(mut d)) => {
            d.push(".khaki");
            d.push("cache");
            Some(d)
        }
        (None, None) => None,
    }
}

fn require_cachedir() -> PathBuf {
    match cachedir() {
        None => {
            eprintln!("Unable to find a usable cache directory!");
            process::exit(1);
        }
        Some(dir) => dir,
    }
}

fn digest_path(path: &Path) -> String {
    let os_str = path.as_os_str();

    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        return digest(os_str.as_bytes());
    }

    #[cfg(windows)]
    {
        // !!! untested
        use std::os::windows::ffi::OsStrExt;
        let bytes = os_str
            .encode_wide()
            .flat_map(|wchar| [(wchar >> 8) as u8, wchar as u8].iter())
            .collect::<Vec<u8>>();
        return digest(&bytes);
    }

    #[cfg(not(any(unix, windows)))]
    {
        return digest(os_str.to_string_lossy().as_bytes());
    }
}

fn digest(stuff: &[u8]) -> String {
    let hash = hmac_sha256::Hash::hash(stuff);
    let mut hex = String::with_capacity(hash.len() * 2);
    for b in &hash {
        write!(hex, "{:02x}", b).unwrap();
    }
    hex
}

fn preprocess_none(input: &fs::File, output_base: &Path) -> io::Result<PathBuf> {
    use io::BufRead;

    let mut processed = String::new();
    let bufreader = io::BufReader::new(input);
    for (number, line) in bufreader.lines().enumerate() {
        let line = line?;
        if number == 0 && line.starts_with("#!/") {
            // assume shebang, so let's drop it. Leave a blank line to preserve line numbers
            writeln!(processed, "").unwrap();
            continue;
        }
        writeln!(processed, "{}", line).unwrap();
    }

    let mut processed_path = PathBuf::from(output_base);
    processed_path.set_extension("rs");
    // TODO: some sort of locking here to avoid concurrently-running `khaki` instances writing to `processed_path`
    fs::write(&processed_path, processed)?;
    Ok(processed_path)
}

fn preprocess_main(input: &fs::File, output_base: &Path) -> io::Result<PathBuf> {
    use io::BufRead;

    let mut has_main = false;
    let mut processed = String::new();
    // TODO: poke rustc folks about source maps or #line macros or such. See rust-lang/rfcs#1573
    let bufreader = io::BufReader::new(input);
    for (number, line) in bufreader.lines().enumerate() {
        let line = line?;
        if number == 0 && line.starts_with("#!/") {
            // assume shebang, so let's drop it
            continue;
        }
        if line.starts_with("fn main(") {
            has_main = true;
        }
        writeln!(processed, "{}", line).unwrap();
    }

    if !has_main {
        processed.insert_str(0, "fn main() {\n");
        processed.push_str("\n}\n");
    }

    processed.insert_str(
        0,
        "\
        #![allow(unused_imports)]\n\
        use std::env::args;\n\
        use std::io::*;\n\
        use std::process::exit;\n\
        \n\
    ",
    );

    let mut processed_path = PathBuf::from(output_base);
    processed_path.set_extension("rs");
    // TODO: some sort of locking here to avoid concurrently-running `khaki` instances writing to `processed_path`
    fs::write(&processed_path, processed)?;
    Ok(processed_path)
}

fn show_cache_dir() -> ! {
    println!("{}", require_cachedir().display());
    process::exit(0);
}

fn clear_cache_dir() -> ! {
    let cachedir = require_cachedir();
    match fs::read_dir(&cachedir) {
        Ok(files) => {
            for entry in files {
                match entry {
                    Err(e) => eprintln!("Error encountered while iterating cache directory {}: {}", cachedir.display(), e),
                    Ok(file) if file.path().is_file() => {
                        match fs::remove_file(file.path()) {
                            Ok(_) => eprintln!("Successfully deleted file {}", file.path().display()),
                            Err(e) => eprintln!("Unable to delete file {}: {}", file.path().display(), e),
                        };
                    }
                    Ok(_nonfile) => continue,
                }
            }
            process::exit(0);
        }
        Err(e) => {
            eprintln!("Unable to read from cache dir {}: {}", cachedir.display(), e);
            process::exit(1);
        }
    }
}

enum PreprocessMode {
    None,
    Main,
}

fn parse_args<T: Iterator<Item = String>>(args: &mut T) -> Option<(String, PreprocessMode)> {
    let mut mode = PreprocessMode::None;
    loop {
        match args.next() {
            None => return None,
            Some(arg) if arg == "--show-cache-dir" => show_cache_dir(),
            Some(arg) if arg == "--clear-cache-dir" => clear_cache_dir(),
            Some(arg) if arg == "--main" => {
                mode = PreprocessMode::Main;
            }
            Some(arg) => return Some((arg, mode)),
        }
    }
}

fn main() {
    let mut args = env::args().skip(1);
    let (script_path, mode) = match parse_args(&mut args) {
        None => {
            usage();
            process::exit(1);
        }
        Some((file, mode)) => {
            (fs::canonicalize(&file).expect(&format!("Unable to get absolute path for {}", &file)), mode)
        }
    };

    let cachedir = cachedir().expect("Unable to find a usable cache directory!");
    fs::create_dir_all(&cachedir).expect(&format!(
        "Error while create cache directory {}",
        cachedir.display()
    ));

    let path_digest = digest_path(&script_path);
    let mut executable = cachedir.clone();
    executable.push(path_digest);

    let khakifile = fs::File::open(&script_path).expect(&format!(
        "Unable to open input script {}",
        script_path.display()
    ));
    let processed_path = match mode {
        PreprocessMode::None => preprocess_none(&khakifile, &executable).unwrap(),
        PreprocessMode::Main => preprocess_main(&khakifile, &executable).unwrap(),
    };

    let compiled = process::Command::new("rustc")
        .arg("-o")
        .arg(&executable)
        .arg(&processed_path)
        .status()
        .expect("Unable to execute rustc")
        .success();
    if !compiled {
        process::exit(3);
    }

    let mut script_cmd = process::Command::new(&executable);
    for arg in args {
        script_cmd.arg(arg);
    }
    let status = script_cmd.status().expect("Unable to execute script");
    match status.code() {
        Some(code) => process::exit(code),
        None => {
            #[cfg(unix)]
            {
                use std::os::unix::process::ExitStatusExt;
                eprintln!(
                    "Script terminated by signal {}",
                    status.signal().unwrap_or(0)
                );
            }
            process::exit(4);
        }
    };
}
