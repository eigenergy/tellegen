//! A small Rust lexer: identifiers, punctuation and literals with their byte
//! offsets and lines, and the comments kept apart, so the scanner can match
//! braces and names without ever looking inside a string or a comment.
//!
//! @block Lexer

/// One token of code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tok {
    /// An identifier or keyword (`r#name` without its prefix).
    Ident(String),
    /// One punctuation character.
    Punct(char),
    /// A string, character or number literal.
    Literal,
    /// A lifetime or loop label.
    Lifetime,
}

/// A token with its position.
#[derive(Clone, Debug)]
pub struct Token {
    pub tok: Tok,
    /// Byte offset of the first character.
    pub start: usize,
    /// Byte offset after the last character.
    pub end: usize,
    /// 1-based line.
    pub line: usize,
}

impl Token {
    pub fn ident(&self) -> Option<&str> {
        match &self.tok {
            Tok::Ident(s) => Some(s),
            _ => None,
        }
    }

    pub fn is(&self, c: char) -> bool {
        self.tok == Tok::Punct(c)
    }
}

/// What a line comment is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommentKind {
    /// `///`: documents the item after it.
    Outer,
    /// `//!`: documents the enclosing module.
    Inner,
    /// `//`: an ordinary comment.
    Plain,
}

/// One line comment, without its marker and the single space after it.
#[derive(Clone, Debug)]
pub struct Comment {
    pub kind: CommentKind,
    pub text: String,
    pub start: usize,
    pub line: usize,
}

/// A lexed file.
pub struct Lexed {
    pub tokens: Vec<Token>,
    pub comments: Vec<Comment>,
}

/// Lex Rust source text. Block comments are skipped (they carry no block
/// tags); unknown characters become punctuation.
#[must_use]
pub fn lex(src: &str) -> Lexed {
    let b = src.as_bytes();
    let mut tokens = Vec::new();
    let mut comments = Vec::new();
    let mut i = 0;
    let mut line = 1;
    let n = b.len();
    // Advance over `from..to`, counting newlines.
    let count = |from: usize, to: usize| b[from..to].iter().filter(|&&c| c == b'\n').count();
    while i < n {
        let c = b[i];
        if c == b'\n' {
            line += 1;
            i += 1;
        } else if c.is_ascii_whitespace() {
            i += 1;
        } else if c == b'/' && b.get(i + 1) == Some(&b'/') {
            let end = src[i..].find('\n').map_or(n, |k| i + k);
            let body = &src[i + 2..end];
            let (kind, text) = if let Some(rest) = body.strip_prefix('!') {
                (CommentKind::Inner, rest)
            } else if body.starts_with('/') && !body.starts_with("//") {
                (CommentKind::Outer, &body[1..])
            } else {
                (CommentKind::Plain, body)
            };
            let text = text.strip_prefix(' ').unwrap_or(text).trim_end();
            comments.push(Comment {
                kind,
                text: text.to_owned(),
                start: i,
                line,
            });
            i = end;
        } else if c == b'/' && b.get(i + 1) == Some(&b'*') {
            // Nested block comment.
            let mut depth = 0usize;
            let mut j = i;
            while j < n {
                if b[j] == b'/' && b.get(j + 1) == Some(&b'*') {
                    depth += 1;
                    j += 2;
                } else if b[j] == b'*' && b.get(j + 1) == Some(&b'/') {
                    depth -= 1;
                    j += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    j += 1;
                }
            }
            line += count(i, j.min(n));
            i = j.min(n);
        } else if let Some(end) = raw_string_end(b, i) {
            tokens.push(Token {
                tok: Tok::Literal,
                start: i,
                end,
                line,
            });
            line += count(i, end);
            i = end;
        } else if c == b'"' || (c == b'b' && b.get(i + 1) == Some(&b'"')) {
            let mut j = if c == b'b' { i + 2 } else { i + 1 };
            while j < n && b[j] != b'"' {
                j += if b[j] == b'\\' { 2 } else { 1 };
            }
            let end = (j + 1).min(n);
            tokens.push(Token {
                tok: Tok::Literal,
                start: i,
                end,
                line,
            });
            line += count(i, end);
            i = end;
        } else if c == b'\'' || (c == b'b' && b.get(i + 1) == Some(&b'\'')) {
            let q = if c == b'b' { i + 1 } else { i };
            if let Some(end) = char_literal_end(src, q) {
                tokens.push(Token {
                    tok: Tok::Literal,
                    start: i,
                    end,
                    line,
                });
                i = end;
            } else {
                // A lifetime or label: `'a`, `'static`, `'outer`.
                let mut j = q + 1;
                while j < n && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
                    j += 1;
                }
                tokens.push(Token {
                    tok: Tok::Lifetime,
                    start: i,
                    end: j,
                    line,
                });
                i = j;
            }
        } else if c.is_ascii_digit() {
            let mut j = i + 1;
            while j < n
                && (b[j].is_ascii_alphanumeric()
                    || b[j] == b'_'
                    || (b[j] == b'.' && b.get(j + 1).is_some_and(u8::is_ascii_digit)))
            {
                // `1e-5`, `2.0e+3`: a sign after an exponent belongs to the number.
                if (b[j] == b'e' || b[j] == b'E')
                    && matches!(b.get(j + 1), Some(b'+' | b'-'))
                    && b[i..j]
                        .iter()
                        .all(|d| d.is_ascii_digit() || *d == b'.' || *d == b'_')
                {
                    j += 1;
                }
                j += 1;
            }
            tokens.push(Token {
                tok: Tok::Literal,
                start: i,
                end: j,
                line,
            });
            i = j;
        } else if c.is_ascii_alphabetic() || c == b'_' {
            let mut s = i;
            if c == b'r' && b.get(i + 1) == Some(&b'#') {
                s = i + 2;
            }
            let mut j = s;
            while j < n && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
                j += 1;
            }
            if j == s {
                // `r#` followed by no identifier: treat `r` alone.
                j = i + 1;
                s = i;
            }
            tokens.push(Token {
                tok: Tok::Ident(src[s..j].to_owned()),
                start: i,
                end: j,
                line,
            });
            i = j;
        } else {
            let ch = src[i..].chars().next().unwrap_or('?');
            let w = ch.len_utf8();
            tokens.push(Token {
                tok: Tok::Punct(ch),
                start: i,
                end: i + w,
                line,
            });
            i += w;
        }
    }
    Lexed { tokens, comments }
}

/// The language of a source file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    Rust,
    /// TypeScript or JavaScript (and the scripts of a Svelte component).
    Ts,
}

/// Lex TypeScript. `/** … */` comments become [`CommentKind::Outer`] lines
/// (one per line, without the leading `*`), `//` lines plain comments;
/// strings, template literals (with their `${…}` parts) and regular
/// expressions are literals.
#[must_use]
pub fn lex_ts(src: &str) -> Lexed {
    let b = src.as_bytes();
    let n = b.len();
    let mut tokens: Vec<Token> = Vec::new();
    let mut comments = Vec::new();
    let mut i = 0;
    let mut line = 1;
    let count = |from: usize, to: usize| b[from..to].iter().filter(|&&c| c == b'\n').count();
    // Whether a `/` here starts a regular expression (not a division).
    let regex_ok = |tokens: &[Token]| match tokens.last() {
        None => true,
        Some(t) => match &t.tok {
            Tok::Punct(c) => !matches!(c, ')' | ']' | '}'),
            Tok::Ident(s) => matches!(
                s.as_str(),
                "return"
                    | "typeof"
                    | "case"
                    | "in"
                    | "of"
                    | "delete"
                    | "void"
                    | "throw"
                    | "new"
                    | "else"
                    | "do"
                    | "yield"
                    | "await"
            ),
            _ => false,
        },
    };
    let push = |tokens: &mut Vec<Token>, tok: Tok, start: usize, end: usize, line: usize| {
        tokens.push(Token {
            tok,
            start,
            end,
            line,
        });
    };
    while i < n {
        let c = b[i];
        if c == b'\n' {
            line += 1;
            i += 1;
        } else if c.is_ascii_whitespace() {
            i += 1;
        } else if c == b'/' && b.get(i + 1) == Some(&b'/') {
            let end = src[i..].find('\n').map_or(n, |k| i + k);
            let body = &src[i + 2..end];
            let text = body.strip_prefix(' ').unwrap_or(body);
            comments.push(Comment {
                kind: CommentKind::Plain,
                text: text.trim_end().to_owned(),
                start: i,
                line,
            });
            i = end;
        } else if c == b'/' && b.get(i + 1) == Some(&b'*') {
            let end = src[i + 2..].find("*/").map_or(n, |k| i + 2 + k + 2);
            let doc = b.get(i + 2) == Some(&b'*') && !matches!(b.get(i + 3), Some(b'*' | b'/'));
            if doc {
                // One comment per line; `/**`, `*/` and a leading `*` removed.
                let body_end = end.saturating_sub(2).max(i + 3).min(n);
                let body = &src[i + 3..body_end];
                let mut off = i + 3;
                for (k, raw) in body.split('\n').enumerate() {
                    let text = if k == 0 {
                        raw.trim()
                    } else {
                        let t = raw.trim_start();
                        let t = t.strip_prefix('*').unwrap_or(t);
                        t.strip_prefix(' ').unwrap_or(t).trim_end()
                    };
                    // `/**` alone on its first line, or `*/` alone on its
                    // last, adds no line.
                    let last = off + raw.len() >= body_end;
                    if !(text.is_empty() && (k == 0 || last)) {
                        comments.push(Comment {
                            kind: CommentKind::Outer,
                            text: text.to_owned(),
                            start: if k == 0 { i } else { off },
                            line: line + k,
                        });
                    }
                    off += raw.len() + 1;
                }
            }
            line += count(i, end.min(n));
            i = end.min(n);
        } else if c == b'"' || c == b'\'' {
            let mut j = i + 1;
            while j < n && b[j] != c && b[j] != b'\n' {
                j += if b[j] == b'\\' { 2 } else { 1 };
            }
            let end = (j + 1).min(n);
            push(&mut tokens, Tok::Literal, i, end, line);
            i = end;
        } else if c == b'`' {
            let end = template_end(b, i);
            push(&mut tokens, Tok::Literal, i, end, line);
            line += count(i, end);
            i = end;
        } else if c == b'/' && regex_ok(&tokens) {
            let mut j = i + 1;
            let mut class = false;
            while j < n && b[j] != b'\n' {
                match b[j] {
                    b'\\' => j += 1,
                    b'[' => class = true,
                    b']' => class = false,
                    b'/' if !class => break,
                    _ => {}
                }
                j += 1;
            }
            j = (j + 1).min(n);
            while j < n && b[j].is_ascii_alphabetic() {
                j += 1;
            }
            push(&mut tokens, Tok::Literal, i, j, line);
            i = j;
        } else if c.is_ascii_digit() {
            let mut j = i + 1;
            while j < n && (b[j].is_ascii_alphanumeric() || b[j] == b'_' || b[j] == b'.') {
                if (b[j] == b'e' || b[j] == b'E') && matches!(b.get(j + 1), Some(b'+' | b'-')) {
                    j += 1;
                }
                j += 1;
            }
            push(&mut tokens, Tok::Literal, i, j, line);
            i = j;
        } else if c.is_ascii_alphabetic() || c == b'_' || c == b'$' {
            let mut j = i + 1;
            while j < n && (b[j].is_ascii_alphanumeric() || b[j] == b'_' || b[j] == b'$') {
                j += 1;
            }
            push(&mut tokens, Tok::Ident(src[i..j].to_owned()), i, j, line);
            i = j;
        } else {
            let ch = src[i..].chars().next().unwrap_or('?');
            let w = ch.len_utf8();
            push(&mut tokens, Tok::Punct(ch), i, i + w, line);
            i += w;
        }
    }
    Lexed { tokens, comments }
}

/// The end of a template literal starting at `i`, past its `${…}` parts
/// (which may hold strings and templates of their own).
fn template_end(b: &[u8], i: usize) -> usize {
    let n = b.len();
    let mut j = i + 1;
    while j < n {
        match b[j] {
            b'\\' => j += 2,
            b'`' => return j + 1,
            b'$' if b.get(j + 1) == Some(&b'{') => {
                let mut depth = 1;
                j += 2;
                while j < n && depth > 0 {
                    match b[j] {
                        b'{' => depth += 1,
                        b'}' => depth -= 1,
                        b'`' => {
                            j = template_end(b, j);
                            continue;
                        }
                        b'"' | b'\'' => {
                            let q = b[j];
                            j += 1;
                            while j < n && b[j] != q {
                                j += if b[j] == b'\\' { 2 } else { 1 };
                            }
                        }
                        _ => {}
                    }
                    j += 1;
                }
            }
            _ => j += 1,
        }
    }
    n
}

/// The parts of a Svelte component the scanner reads, at their offsets: the
/// `<script>` contents as they are, the markup's `{…}` expressions, and the
/// name of every component the markup uses (`<ZoomChart`). Everything else
/// (text, attributes, styles, HTML comments) becomes spaces, newlines kept.
#[must_use]
pub fn svelte_code(src: &str) -> String {
    let b = src.as_bytes();
    let n = b.len();
    let mut out: Vec<u8> = b
        .iter()
        .map(|&c| if c == b'\n' { b'\n' } else { b' ' })
        .collect();
    let keep = |out: &mut Vec<u8>, from: usize, to: usize| {
        out[from..to].copy_from_slice(&b[from..to]);
    };
    let mut i = 0;
    while i < n {
        let rest = &src[i..];
        if rest.starts_with("<script") {
            let open_end = rest.find('>').map_or(n, |k| i + k + 1);
            let close = src[open_end..]
                .find("</script>")
                .map_or(n, |k| open_end + k);
            keep(&mut out, open_end, close);
            i = (close + "</script>".len()).min(n);
        } else if rest.starts_with("<style") {
            i = rest
                .find("</style>")
                .map_or(n, |k| i + k + "</style>".len());
        } else if rest.starts_with("<!--") {
            i = rest.find("-->").map_or(n, |k| i + k + 3);
        } else if b[i] == b'<' && b.get(i + 1).is_some_and(u8::is_ascii_uppercase) {
            let mut j = i + 1;
            while j < n && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
                j += 1;
            }
            keep(&mut out, i + 1, j);
            i = j;
        } else if b[i] == b'{' {
            // A markup expression, braces balanced.
            let mut depth = 0;
            let mut j = i;
            while j < n {
                match b[j] {
                    b'{' => depth += 1,
                    b'}' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    b'`' => {
                        j = template_end(b, j);
                        continue;
                    }
                    _ => {}
                }
                j += 1;
            }
            let end = (j + 1).min(n);
            keep(&mut out, i, end);
            i = end;
        } else {
            i += src[i..].chars().next().map_or(1, char::len_utf8);
        }
    }
    // Only ASCII spaces replaced whole characters' bytes; a kept range
    // always starts and ends on a character boundary.
    String::from_utf8(out).unwrap_or_default()
}

/// The end of a raw string (`r"…"`, `r#"…"#`, `br#"…"#`) starting at `i`.
fn raw_string_end(b: &[u8], i: usize) -> Option<usize> {
    let mut j = i;
    if b.get(j) == Some(&b'b') {
        j += 1;
    }
    if b.get(j) != Some(&b'r') {
        return None;
    }
    // Not part of an identifier like `for` or `bar`.
    if i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_') {
        return None;
    }
    j += 1;
    let mut hashes = 0;
    while b.get(j) == Some(&b'#') {
        hashes += 1;
        j += 1;
    }
    if b.get(j) != Some(&b'"') {
        return None;
    }
    j += 1;
    while j < b.len() {
        if b[j] == b'"' && (0..hashes).all(|k| b.get(j + 1 + k) == Some(&b'#')) {
            return Some(j + 1 + hashes);
        }
        j += 1;
    }
    Some(b.len())
}

/// The end of a character literal whose opening quote is at `q`, or `None`
/// for a lifetime.
fn char_literal_end(src: &str, q: usize) -> Option<usize> {
    let rest = &src[q + 1..];
    let mut chars = rest.char_indices();
    let (_, first) = chars.next()?;
    if first == '\\' {
        // Escape (`'\n'`, `'\''`, `'\u{41}'`): the closing quote after the
        // escaped character.
        let close = rest.get(2..)?.find('\'')? + 2;
        return Some(q + 1 + close + 1);
    }
    let (k, second) = chars.next()?;
    (second == '\'').then_some(q + 1 + k + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn idents(src: &str) -> Vec<String> {
        lex(src)
            .tokens
            .iter()
            .filter_map(|t| t.ident().map(str::to_owned))
            .collect()
    }

    #[test]
    fn strings_and_comments_hide_their_contents() {
        let src = "let a = \"fn hidden {\"; // fn nope\nlet b = r#\"x \" y\"#; /* fn no */ 'c';";
        assert_eq!(idents(src), ["let", "a", "let", "b"]);
        let lexed = lex(src);
        assert_eq!(lexed.comments.len(), 1);
        assert_eq!(lexed.comments[0].text, "fn nope");
    }

    #[test]
    fn lifetimes_are_not_char_literals() {
        let lexed = lex("fn f<'a>(x: &'a str) -> char { 'x' }");
        let lifetimes = lexed
            .tokens
            .iter()
            .filter(|t| t.tok == Tok::Lifetime)
            .count();
        assert_eq!(lifetimes, 2);
        assert!(lexed.tokens.iter().any(|t| t.tok == Tok::Literal));
    }

    #[test]
    fn doc_comment_kinds_and_lines() {
        let lexed = lex("//! inner\n/// outer\n//// plain\nfn x() {}\n");
        let kinds: Vec<_> = lexed.comments.iter().map(|c| (c.kind, c.line)).collect();
        assert_eq!(
            kinds,
            [
                (CommentKind::Inner, 1),
                (CommentKind::Outer, 2),
                (CommentKind::Plain, 3)
            ]
        );
        assert_eq!(lexed.tokens[0].line, 4);
    }

    #[test]
    fn exponents_stay_in_numbers() {
        let lexed = lex("let t = 1e-9 * h; let r = 0..n;");
        assert!(idents("let t = 1e-9 * h;").contains(&"h".to_owned()));
        assert!(lexed.tokens.iter().any(|t| t.ident() == Some("n")));
    }
}
