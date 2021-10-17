use std::io::stdin;

/// Read one line from stdin, and returns the first non-whitespace char.
pub fn read_one_char() -> Option<char> {
    let mut line = String::new();
    stdin().read_line(&mut line).expect("failed to read from stdin");
    line.trim().chars().next()
}
