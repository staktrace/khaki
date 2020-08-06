khaki
===
A tool that lets you write quick-and-dirty rust "scripts" with less boilerplate.

Example usage
---

```
$ cargo install khaki
$ cat examples/echo.rs | examples/echo.rs
#!/usr/bin/env khaki

let stdin = stdin();
loop {
    let mut buf = String::new();
    match stdin.read_line(&mut buf) {
        Ok(0) => break,
        Ok(_) => print!("{}", buf),
        _ => exit(1),
    };
}
```

In case it's not obvious from the above example, the `examples/echo.rs` file is
standard Rust code, but without the boilerplate of a `main` function and `use`
statements. And it's an executable (`chmod +x`) file with a shebang line that
runs it via the `khaki` "interpreter". The "interpreter" really just preprocesses
the script to insert the missing boilerplate, and then compiles it with `rustc`
and executes it.

Goals
---
The overarching goal of `khaki` is to make it as easy to use Rust in one-off
scripts as it is to use Python.

FAQ
---
Q: Why the name "khaki"?
A: It's a color that's lighter than rust.
