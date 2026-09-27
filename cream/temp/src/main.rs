use regex::Regex;
use std::borrow::Cow;
use std::io::{self, IsTerminal, Read};
use std::path::Path;
use std::sync::LazyLock;

#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)]
struct PathMatch {
    path: String,
    line_num: Option<u32>,
    col_num: Option<u32>,
    start: usize,
    end: usize,
}

#[allow(dead_code)]
const TRAILING_PUNCT: &[char] = &['!', '.', ',', ';', ':', ')', ']', '}', '>', '"', '\''];

#[allow(dead_code)]
fn expand_tilde(path: &str) -> Cow<'_, str> {
    match path.strip_prefix("~/") {
        Some(rest) => match std::env::var("HOME") {
            Ok(home) => Cow::Owned(format!("{home}/{rest}")),
            Err(_) => Cow::Borrowed(path),
        },
        None => Cow::Borrowed(path),
    }
}

/// Longest prefix of `raw` that names an existing file, after trimming
/// trailing sentence punctuation the regex can't tell apart from the path.
#[allow(dead_code)]
fn validate_path(raw: &str) -> Option<usize> {
    if raw.trim_matches('/').is_empty() {
        return None;
    }
    let mut len = raw.len();
    loop {
        let candidate = &raw[..len];
        if Path::new(expand_tilde(candidate).as_ref()).exists() {
            return Some(len);
        }
        let last = candidate.chars().next_back().unwrap();
        if len > 1 && TRAILING_PUNCT.contains(&last) {
            len -= last.len_utf8();
        } else {
            return None;
        }
    }
}

#[allow(dead_code)]
fn extract_path_matches(input: &str) -> Vec<PathMatch> {
    static CANDIDATE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?P<path>[^\s:/]*/[^\s:]*|(?:[A-Za-z0-9_+~-]+\.)+[A-Za-z0-9_+~-]+)(?::(?P<line_num>[0-9]+)(?::(?P<col_num>[0-9]+))?)?",
        )
        .unwrap()
    });

    let mut matches = Vec::new();
    for caps in CANDIDATE.captures_iter(input) {
        let m = caps.name("path").unwrap();
        let raw = m.as_str();
        let Some(valid_len) = validate_path(raw) else {
            continue;
        };

        let (line_num, col_num, end) = if valid_len == raw.len() {
            (
                caps.name("line_num")
                    .map(|l| l.as_str().parse::<u32>().unwrap()),
                caps.name("col_num")
                    .map(|c| c.as_str().parse::<u32>().unwrap()),
                caps.get(0).unwrap().end(),
            )
        } else {
            // A :line_num suffix glued after trimmed punctuation isn't trusted.
            (None, None, m.start() + valid_len)
        };

        matches.push(PathMatch {
            path: raw[..valid_len].to_string(),
            line_num,
            col_num,
            start: m.start(),
            end,
        });
    }
    matches
}

fn main() {
    let mut text = String::new();
    if !io::stdin().is_terminal() {
        io::stdin().read_to_string(&mut text).unwrap();
    } else {
        text = " 
        This is a file src/main.rs:some content \
        Cargo.toml:15:[package] \
        Cargo.toml:15:98[package] \
        Cargo.toml:15:98:11[package] \
        "
        .to_string();
    }

    let res: Vec<PathMatch> = extract_path_matches(text.as_str());
    for r in res {
        println!(
            "Path is: {}, line num: {:?}, col num: {:?}",
            r.path, r.line_num, r.col_num
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
}
