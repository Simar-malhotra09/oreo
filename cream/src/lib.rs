use regex::Regex;
use std::borrow::Cow;
use std::fmt::{self, Display, Formatter};
use std::path::Path;
use std::sync::LazyLock;

// this should be one 'chunk' of text
// say, newline delimited for now.
#[derive(Default)]
pub struct Packed<'out> {
    pub chunk: &'out str,
    pub path_match: PathMatch,
}

#[derive(Default)]
pub struct ChunkPathPairs<'out> {
    pub pairs: Vec<Packed<'out>>,
}

impl<'out> ChunkPathPairs<'out> {
    pub fn new(content: &'out str) -> Self {
        let items = content // items expected to be Vec<Packed>
            .split('\n')
            .flat_map(Packed::new) //inner returns Vec<Packed>, so I have //Vec<Vec<Packed>>
            // essentially, i has lifeitme of content, every other ref used should have the same
            .collect();
        Self { pairs: items }
    }
    // pub fn new_single_chunk(mut content: String) -> Self {
    //     let items = ChunkPathPairs::strip_newlines(&mut content)
    //         .split('\n')
    //         .flat_map(|i| Packed::new(i))
    //         .collect();
    //     Self { pairs: items }
    // }

    #[allow(dead_code)]
    fn strip_newlines(content: &mut String) -> &String {
        *content = content.replace('\n', " ");
        content
    }
}

impl<'out> Display for ChunkPathPairs<'out> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        for Packed { chunk, path_match } in &self.pairs {
            writeln!(f, "chunk: {}", chunk)?;
            writeln!(f, "matches: {:?}", path_match)?;
            writeln!(f, "{}", "-".repeat(10))?;
        }

        Ok(())
    }
}

impl<'out> Packed<'out> {
    pub fn new(content: &'out str) -> Vec<Self> {
        let matches = extract_path_matches(content);
        let mut res = Vec::<Packed>::new();
        for path_match in matches {
            res.push(Self {
                chunk: content,
                path_match,
            })
        }
        res
    }
    pub fn does_content_have_newlines(content: &str) -> bool {
        content.contains('\n')
    }
}

#[derive(Default)]
pub struct Output<'out> {
    pub o_stdin: ChunkPathPairs<'out>,
    pub o_stdout: ChunkPathPairs<'out>,
    pub o_stderr: ChunkPathPairs<'out>,
}

#[derive(Default, Debug, PartialEq, Eq)]
pub struct PathMatch {
    pub path: String,
    pub line_num: Option<u32>,
    pub col_num: Option<u32>,
    pub start: usize,
    pub end: usize,
}
impl fmt::Display for PathMatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}",
            self.path,
            self.line_num.map_or("-".to_string(), |n| n.to_string()),
            self.col_num.map_or("-".to_string(), |n| n.to_string()),
        )
    }
}

const TRAILING_PUNCT: &[char] = &['.', ',', ';', ':', ')', ']', '}', '>', '"', '\'', '!'];

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

pub fn extract_path_matches(input: &str) -> Vec<PathMatch> {
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
            // println!("{} isn't valid! ", raw);
            continue;
        };

        let (line_num, col_num, end) = if valid_len == raw.len() {
            (
                caps.name("line_num").map(|l| l.as_str().parse().unwrap()),
                caps.name("col_num").map(|c| c.as_str().parse().unwrap()),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn found(input: &str) -> Vec<(String, Option<u32>, Option<u32>)> {
        extract_path_matches(input)
            .into_iter()
            .map(|m| (m.path, m.line_num, m.col_num))
            .collect()
    }

    #[test]
    fn test_expand_path_valid() {
        let path = "~/Desktop";
        let expanded_path = expand_tilde(path);
        // it doesn't have the trailing '/'
        assert_eq!(expanded_path, "/Users/0saker/Desktop");
    }

    #[test]
    fn test_expand_path_invalid() {
        let path = "~//Desktop";
        let expanded_path = expand_tilde(path);
        // it doesn't have the trailing '/'
        assert_eq!(expanded_path, "/Users/0saker//Desktop");
    }

    // Todo: decide what to do
    #[test]
    fn test_expand_path_multiple_tilde() {
        let path = "~~/Desktop";
        let expanded_path = expand_tilde(path);
        assert_eq!(expanded_path, "~~/Desktop");
    }
    #[test]
    fn test_validate_path_valid() {
        let path = "~/Desktop";
        let expected_size: usize = path.len();
        assert_eq!(validate_path(path).unwrap(), expected_size);
    }
    #[test]
    fn test_validate_path_invalid_multiple_tilde() {
        let path = "~~/Desktop";
        assert!(validate_path(path).is_none());
    }
    #[test]
    fn test_validate_path_invalid_dne() {
        let path = "~/abc/def";
        assert!(validate_path(path).is_none());
    }

    #[test]
    fn test_validate_path_trailing_punc_single() {
        let path = "~/Desktop,";
        let expected_size: usize = "~/Desktop".len();
        assert_eq!(validate_path(path).unwrap(), expected_size);
    }

    #[test]
    fn test_validate_path_trailing_punc_multiple() {
        let path = "~/Desktop,!";
        let expected_size: usize = "~/Desktop".len();
        assert_eq!(validate_path(path).unwrap(), expected_size);
    }

    #[test]
    fn test_extract_path_matches_path() {
        let path = "~/Desktop";
        assert_eq!(found(path), vec![("~/Desktop".to_string(), None, None)])
    }
    #[test]
    fn test_extract_path_matches_path_and_line_num() {
        let path = "~/Desktop:12";
        assert_eq!(found(path), vec![("~/Desktop".to_string(), Some(12), None)])
    }
    #[test]
    fn test_extract_path_matches_path_and_line_num_and_col_num() {
        let path = "~/Desktop:12:10";
        assert_eq!(
            found(path),
            vec![("~/Desktop".to_string(), Some(12), Some(10))]
        )
    }
    #[test]
    fn test_extract_path_matches_path_trailing_col() {
        let path = "~/Desktop:";
        assert_eq!(found(path), vec![("~/Desktop".to_string(), None, None)])
    }
    #[test]
    fn test_extract_path_matches_path_trailing_col_multiple() {
        let path = "~/Desktop::;";
        assert_eq!(found(path), vec![("~/Desktop".to_string(), None, None)])
    }
    #[test]
    fn test_extract_path_matches_colon_chars() {
        let path = "~/Desktop:10:abc";
        assert_eq!(found(path), vec![("~/Desktop".to_string(), Some(10), None)])
    }

    #[test]
    fn fixture_bare_path_no_line_number() {
        assert_eq!(
            found(include_str!(
                "../tests/fixtures/bare_path_no_line_number.txt"
            )),
            vec![("src/main.rs".to_string(), None, None)]
        );
    }

    #[test]
    fn fixture_path_with_line_number() {
        assert_eq!(
            found(include_str!("../tests/fixtures/path_with_line_number.txt")),
            vec![("src/main.rs".to_string(), Some(42), None)]
        );
    }

    #[test]
    fn fixture_mixed_multiline() {
        assert_eq!(
            found(include_str!("../tests/fixtures/mixed_multiline.txt")),
            vec![
                ("src/main.rs".to_string(), None, None),
                ("Cargo.toml".to_string(), Some(15), None),
            ]
        );
    }

    #[test]
    fn fixture_no_path_prose() {
        assert!(found(include_str!("../tests/fixtures/no_path_prose.txt")).is_empty());
    }

    #[test]
    fn absolute_paths() {
        let abs = std::env::current_dir()
            .unwrap()
            .join("src/main.rs")
            .to_string_lossy()
            .into_owned();
        assert_eq!(
            found(&format!("{abs}:some content")),
            vec![(abs.clone(), None, None)]
        );
        assert_eq!(
            found(&format!("{abs}:42:some content")),
            vec![(abs, Some(42), None)]
        );
    }

    #[test]
    fn trailing_punctuation_is_trimmed() {
        assert_eq!(
            found("edit src/main.rs, then Cargo.toml."),
            vec![
                ("src/main.rs".to_string(), None, None),
                ("Cargo.toml".to_string(), None, None),
            ]
        );
    }

    #[test]
    fn line_and_col() {
        assert_eq!(
            found("src/main.rs:7:3:uh oh"),
            vec![("src/main.rs".to_string(), Some(7), Some(3))]
        );
    }

    #[test]
    fn span_offsets_cover_path_and_suffix() {
        let input = "see src/main.rs:42 now";
        let m = &extract_path_matches(input)[0];
        assert_eq!(&input[m.start..m.end], "src/main.rs:42");
    }

    #[test]
    fn tilde_expansion() {
        let home = std::env::var("HOME").unwrap();
        assert_eq!(expand_tilde("~/x").as_ref(), format!("{home}/x").as_str());
        assert_eq!(expand_tilde("/abs/x").as_ref(), "/abs/x");
    }

    #[test]
    fn made_up_paths_are_dropped() {
        assert!(found("read notreal.xyz or e.g. whatever").is_empty());
    }

    // #[test]
    // fn
}
