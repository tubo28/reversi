use std::io::stdin;

/// Read one line from stdin, and returns the first non-whitespace char.
pub fn read_one_char() -> Option<char> {
    let mut line = String::new();
    stdin().read_line(&mut line).expect("failed to read from stdin");
    line.trim().chars().next()
}

/// Names positions like A1, A2, ..., B1, B2, ...
/// Columns A through Z by left to right, and rows 1 to 8 by top to bottom.
pub fn position_to_name(r: usize, c: usize) -> String {
    let col_name: Vec<_> = "ABCDEFGH".chars().collect();
    let row_name: Vec<_> = "12345678".chars().collect();
    format!("{}{}", col_name[c], row_name[r])
}
