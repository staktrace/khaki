#!/usr/bin/env krust

let stdin = std::io::stdin();
loop {
    let mut buf = String::new();
    match stdin.read_line(&mut buf) {
        Ok(0) => break,
        Ok(_) => print!("{}", buf),
        _ => std::process::exit(1),
    };
}
