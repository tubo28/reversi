use std::io::stdin;

/// 標準入力から 1 行読み込み，空白文字を除いた先頭の文字を返します．
pub fn read_one_char() -> Option<char> {
    let mut line = String::new();
    stdin()
        .read_line(&mut line)
        .expect("failed to read from stdin");
    line.trim().chars().next()
}
