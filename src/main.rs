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
    eprintln!("  khaki path-to-script.rs [args to script]");
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

fn preprocess(input: &fs::File, output_base: &Path) -> io::Result<PathBuf> {
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
        #![allow(unused_imports)]
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

fn parse_args<T: Iterator<Item = String>>(args: &mut T) -> Option<String> {
    loop {
        match args.next() {
            None => return None,
            Some(arg) if arg == "--show-cache-dir" => {
                match cachedir() {
                    Some(dir) => {
                        println!("{}", dir.display());
                        process::exit(0);
                    }
                    None => {
                        eprintln!("Unable to find a usable cache directory!");
                        process::exit(1);
                    }
                }
            }
            Some(arg) => return Some(arg),
        }
    }
}

fn main() {
    let mut args = env::args().skip(1);
    let script_path = match parse_args(&mut args) {
        None => {
            usage();
            process::exit(1);
        }
        Some(file) => {
            fs::canonicalize(&file).expect(&format!("Unable to get absolute path for {}", &file))
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
    let processed_path = preprocess(&khakifile, &executable).unwrap();

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
