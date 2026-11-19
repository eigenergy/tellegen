//! Writing scanned blocks out: as JSON, and as one self-contained HTML page
//! that draws them as an interactive block diagram (no network access, no
//! external scripts).
//!
//! @block Output

use std::fmt::Write as _;

use crate::scan::Doc;

const VIEWER: &str = include_str!("viewer.html");

/// What the page says about where it came from.
#[derive(Clone, Debug, Default)]
pub struct Meta {
    /// A URL prefix that a file path completes to its source (for example
    /// `https://github.com/org/repo/blob/<rev>/`); empty for none.
    pub source_url: String,
    /// A note on what was scanned (a revision, a date); empty for none.
    pub generated_from: String,
    /// The documentation of the top level when several roots share it.
    pub intro: String,
}

/// The blocks, edges, files and issues as JSON.
#[must_use]
pub fn to_json(doc: &Doc, meta: &Meta) -> String {
    let mut s = String::new();
    s.push('{');
    field(&mut s, "title", &doc.title);
    s.push(',');
    field(
        &mut s,
        "generator",
        concat!("tellegen-blocks ", env!("CARGO_PKG_VERSION")),
    );
    s.push(',');
    field(&mut s, "source_url", &meta.source_url);
    s.push(',');
    field(&mut s, "generated_from", &meta.generated_from);
    s.push(',');
    field(&mut s, "intro", &meta.intro);
    s.push_str(",\"files\":[");
    for (k, f) in doc.files.iter().enumerate() {
        if k > 0 {
            s.push(',');
        }
        s.push('{');
        field(&mut s, "path", &f.path);
        s.push(',');
        field(&mut s, "module", &f.module);
        s.push(',');
        field(&mut s, "text", &f.text);
        s.push('}');
    }
    s.push_str("],\"blocks\":[");
    for (k, b) in doc.blocks.iter().enumerate() {
        if k > 0 {
            s.push(',');
        }
        s.push('{');
        field(&mut s, "id", &b.id);
        let _ = write!(
            s,
            ",\"parent\":{},\"file\":{},\"lines\":[{},{}],\"order\":{},",
            b.parent.map_or("null".to_owned(), |p| p.to_string()),
            b.file.map_or("null".to_owned(), |f| f.to_string()),
            b.start_line,
            b.end_line,
            b.order.min(1 << 52),
        );
        field(&mut s, "kind", b.kind.as_str());
        s.push(',');
        match &b.item {
            Some(item) => field(&mut s, "item", item),
            None => s.push_str("\"item\":null"),
        }
        s.push(',');
        field(&mut s, "title", &b.title);
        s.push(',');
        field(&mut s, "doc", &b.doc);
        s.push(',');
        match &b.signature {
            Some(sig) => field(&mut s, "signature", sig),
            None => s.push_str("\"signature\":null"),
        }
        s.push_str(",\"defines\":");
        strings(&mut s, &b.defines);
        s.push('}');
    }
    s.push_str("],\"edges\":[");
    for (k, e) in doc.edges.iter().enumerate() {
        if k > 0 {
            s.push(',');
        }
        let _ = write!(
            s,
            "{{\"from\":{},\"to\":{},\"count\":{},\"names\":",
            e.from, e.to, e.count
        );
        strings(&mut s, &e.names);
        s.push('}');
    }
    s.push_str("],\"issues\":[");
    for (k, i) in doc.issues.iter().enumerate() {
        if k > 0 {
            s.push(',');
        }
        s.push('{');
        field(&mut s, "file", &i.file);
        let _ = write!(s, ",\"line\":{},", i.line);
        field(&mut s, "message", &i.message);
        s.push('}');
    }
    s.push_str("]}");
    s
}

/// The interactive block diagram: one HTML file with the data inside.
#[must_use]
pub fn to_html(doc: &Doc, meta: &Meta) -> String {
    // Inside <script>, `</` would end the element; `<\/` is the same JSON.
    let data = to_json(doc, meta).replace("</", "<\\/");
    VIEWER
        .replacen("__TITLE__", &html_escape(&doc.title), 1)
        .replacen("__DATA__", &data, 1)
}

fn field(s: &mut String, key: &str, value: &str) {
    string(s, key);
    s.push(':');
    string(s, value);
}

fn strings(s: &mut String, values: &[String]) {
    s.push('[');
    for (k, v) in values.iter().enumerate() {
        if k > 0 {
            s.push(',');
        }
        string(s, v);
    }
    s.push(']');
}

fn string(s: &mut String, v: &str) {
    s.push('"');
    for c in v.chars() {
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            '\n' => s.push_str("\\n"),
            '\r' => s.push_str("\\r"),
            '\t' => s.push_str("\\t"),
            '\u{2028}' => s.push_str("\\u2028"),
            '\u{2029}' => s.push_str("\\u2029"),
            c if (c as u32) < 0x20 => {
                let _ = write!(s, "\\u{:04x}", c as u32);
            }
            c => s.push(c),
        }
    }
    s.push('"');
}

fn html_escape(v: &str) -> String {
    v.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::build;

    #[test]
    fn json_escapes_and_html_embeds() {
        let src = "//! Says \"hi\" </script>\n//! @block Root\n".to_owned();
        let doc = build("T<1>", vec![("m.rs".into(), vec!["m".into()], src)]);
        let json = to_json(&doc, &Meta::default());
        assert!(json.contains(r#"Says \"hi\" </script>"#));
        let html = to_html(&doc, &Meta::default());
        assert!(html.contains("<title>T&lt;1&gt;</title>"));
        assert!(html.contains(r"<\/script>"));
        assert!(!html.contains("__DATA__"));
    }
}
