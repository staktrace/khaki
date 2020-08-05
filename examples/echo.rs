#!/usr/bin/env krust

let stdin = stdin();
loop {
    let mut buf = String::new();
    match stdin.read_line(&mut buf) {
        Ok(0) => break,
        Ok(_) => print!("{}", buf),
        _ => exit(1),
    };
}
