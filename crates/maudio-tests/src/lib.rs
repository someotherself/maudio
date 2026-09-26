pub mod assets;

#[macro_export]
macro_rules! check {
    ($failures:ident, $test:path) => {{
        eprint!("RUN  {} ... ", stringify!($test));
        std::io::Write::flush(&mut std::io::stderr()).unwrap();

        match $test() {
            Ok(()) => eprintln!("PASS"),
            Err(err) => {
                eprintln!();
                eprintln!("FAIL {}: {err:?}", stringify!($test));
                $failures += 1;
            }
        }
    }};
}
