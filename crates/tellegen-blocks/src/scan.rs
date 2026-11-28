//! Reading the blocks of a module tree: which code each block holds, what
//! its documentation says, and which other blocks its code uses.
//!
//! A block is declared where the code is:
//!
//! - a **module** block by `@block <title>` in the `//!` docs of a file; it
//!   holds the whole file;
//! - an **item** block by a `// @block <title>` line right under the `///`
//!   docs of a `fn`, `struct`, `enum`, `trait`, `impl`, `type`, `const`,
//!   `static` or `mod` (so rustdoc and JSON schemas never show the tag; a
//!   `@block` line inside the `///` docs works too); it holds that item;
//! - a **section** block by an own-line `// @block <title>` comment, followed
//!   by `//` lines that document it; it holds the code from there to the next
//!   section marker at the same brace depth, a `// @end` line, or the brace
//!   that closes the enclosing block, whichever comes first.
//!
//! A block's id is its Rust path (`rms::integrate::Stepper::step`, a section
//! `…#title`), its parent the smallest block around it, and the parent of a
//! module block the block of the module that declares it. Its edges come from
//! the code alone: a name used in one block and defined in another (a type, a
//! function, a method, a constant, a module) is an edge between the two.
//! Code under `#[cfg(test)]` and `use` declarations count for neither.
//!
//! @block Scanner

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::path::{Path, PathBuf};

use crate::lex::{lex, lex_ts, svelte_code, Comment, CommentKind, Lang, Lexed, Tok, Token};

/// What declares a block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Module,
    Item,
    Section,
}

impl Kind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Module => "module",
            Kind::Item => "item",
            Kind::Section => "section",
        }
    }
}

/// One documented block of code.
#[derive(Clone, Debug)]
pub struct Block {
    /// The Rust path of the block (`rms::integrate::Stepper`), with
    /// `#section-title` for a section.
    pub id: String,
    pub parent: Option<usize>,
    pub kind: Kind,
    /// The item keyword of an item block (`fn`, `struct`, `impl`, …).
    pub item: Option<String>,
    pub title: String,
    /// The documentation, Markdown, without its tags.
    pub doc: String,
    /// Index into [`Doc::files`]; `None` for a [`Heading`] (a group of
    /// modules without a file of its own).
    pub file: Option<usize>,
    pub start_line: usize,
    pub end_line: usize,
    /// The item's header (`pub fn step(&mut self, …) -> Result<…>`).
    pub signature: Option<String>,
    /// Names this block defines itself (not in a sub-block).
    pub defines: Vec<String>,
    /// Position among its siblings (for module blocks the position of their
    /// `mod` declaration in the parent's file).
    pub order: usize,
    start: usize,
    end: usize,
}

/// "`from` uses `to`": names used in `from`'s own code and defined in `to`'s.
#[derive(Clone, Debug)]
pub struct Edge {
    pub from: usize,
    pub to: usize,
    pub count: usize,
    pub names: Vec<String>,
}

/// A problem with the block annotations.
#[derive(Clone, Debug)]
pub struct Issue {
    pub file: String,
    pub line: usize,
    pub message: String,
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.file, self.line, self.message)
    }
}

/// A scanned source file.
#[derive(Clone, Debug)]
pub struct SourceFile {
    /// The path as shown (relative to the base given to [`scan`]).
    pub path: String,
    /// The module path (`rms::integrate`, `app::logic::live`).
    pub module: String,
    pub text: String,
    pub lang: Lang,
}

/// A directory scanned as module `module`: a Rust module tree (`mod.rs`
/// or `lib.rs` at its root), or a folder of TypeScript and Svelte files,
/// each file a module named after it.
#[derive(Clone, Debug)]
pub struct Root {
    pub dir: PathBuf,
    pub module: String,
    /// The files to scan, as paths relative to `dir` (`rms/threads.rs`);
    /// empty: every file under it.
    pub only: Vec<String>,
}

/// A group of modules without a file of its own (a folder of TypeScript, or
/// the parent of several roots): its module path, title and documentation.
#[derive(Clone, Debug)]
pub struct Heading {
    pub module: String,
    pub title: String,
    pub doc: String,
}

/// The blocks of a module tree.
#[derive(Clone, Debug)]
pub struct Doc {
    pub title: String,
    pub files: Vec<SourceFile>,
    pub blocks: Vec<Block>,
    pub edges: Vec<Edge>,
    pub issues: Vec<Issue>,
}

impl Doc {
    /// The blocks without a parent.
    #[must_use]
    pub fn roots(&self) -> Vec<usize> {
        (0..self.blocks.len())
            .filter(|&b| self.blocks[b].parent.is_none())
            .collect()
    }

    /// The block with this id.
    #[must_use]
    pub fn find(&self, id: &str) -> Option<&Block> {
        self.blocks.iter().find(|b| b.id == id)
    }
}

/// Scan the module tree whose root is `dir` (holding `mod.rs`, or `lib.rs`
/// for a crate root) as module `module`, showing paths relative to `base`.
///
/// # Errors
/// When a file cannot be read.
pub fn scan(dir: &Path, module: &str, base: &Path, title: &str) -> std::io::Result<Doc> {
    let root = Root {
        dir: dir.to_owned(),
        module: module.to_owned(),
        only: Vec::new(),
    };
    scan_roots(&[root], &[], base, title)
}

/// Scan several roots into one diagram; `headings` name the module paths
/// that have no file (`app` above `app::logic`, or a TypeScript folder).
///
/// # Errors
/// When a file cannot be read.
pub fn scan_roots(
    roots: &[Root],
    headings: &[Heading],
    base: &Path,
    title: &str,
) -> std::io::Result<Doc> {
    let mut files = Vec::new();
    for root in roots {
        let mut paths = Vec::new();
        collect(&root.dir, &mut paths)?;
        for path in paths {
            let rel = path.strip_prefix(&root.dir).unwrap_or(&path);
            if !root.only.is_empty() {
                let shown = rel.to_string_lossy().replace('\\', "/");
                if !root.only.contains(&shown) {
                    continue;
                }
            }
            let text = std::fs::read_to_string(&path)?;
            let mut segs: Vec<String> = root.module.split("::").map(str::to_owned).collect();
            let comps: Vec<String> = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            for (k, c) in comps.iter().enumerate() {
                let last = k + 1 == comps.len();
                let c = if last { stem(c) } else { c.as_str() };
                let rust_root = last && path.extension().is_some_and(|e| e == "rs");
                if !(rust_root && (c == "mod" || c == "lib")) {
                    segs.push(c.to_owned());
                }
            }
            let shown = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            files.push((shown, segs, text));
        }
    }
    // Parents before children, so a module block finds its parent's block.
    files.sort_by(|a, b| a.1.len().cmp(&b.1.len()).then(a.1.cmp(&b.1)));
    Ok(build_with(title, files, headings))
}

/// A file's module name: without `.rs`, `.svelte.ts`, `.ts` or `.svelte`.
fn stem(name: &str) -> &str {
    [".svelte.ts", ".ts", ".svelte", ".rs"]
        .iter()
        .find_map(|s| name.strip_suffix(s))
        .unwrap_or(name)
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|e| e.path())
        .collect();
    entries.sort();
    for p in entries {
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if p.is_dir() {
            collect(&p, out)?;
        } else if name.ends_with(".rs")
            || name.ends_with(".svelte")
            || (name.ends_with(".ts")
                && !name.ends_with(".d.ts")
                && !name.ends_with(".test.ts")
                && !name.ends_with(".spec.ts"))
        {
            out.push(p);
        }
    }
    Ok(())
}

/// Scan source texts given directly: `(shown path, module path, text)`,
/// parents before children. The language follows the path's extension.
#[must_use]
pub fn build(title: &str, inputs: Vec<(String, Vec<String>, String)>) -> Doc {
    build_with(title, inputs, &[])
}

fn build_with(
    title: &str,
    inputs: Vec<(String, Vec<String>, String)>,
    headings: &[Heading],
) -> Doc {
    let mut doc = Doc {
        title: title.to_owned(),
        files: Vec::new(),
        blocks: Vec::new(),
        edges: Vec::new(),
        issues: Vec::new(),
    };
    let mut parsed: Vec<FileScan> = Vec::new();
    for (path, module, text) in inputs {
        let fi = doc.files.len();
        let lang = if path.ends_with(".rs") {
            Lang::Rust
        } else {
            Lang::Ts
        };
        let svelte = path.ends_with(".svelte");
        doc.files.push(SourceFile {
            path,
            module: module.join("::"),
            text,
            lang,
        });
        let scan = FileScan::new(&doc.files[fi].text, module, fi, lang, svelte);
        parsed.push(scan);
    }
    for fs in &mut parsed {
        fs.blocks(&mut doc);
    }
    // Headings: blocks for module paths without a file.
    let mut synthetic: Vec<(Vec<String>, usize)> = Vec::new();
    for (k, h) in headings.iter().enumerate() {
        let path: Vec<String> = h.module.split("::").map(str::to_owned).collect();
        if parsed.iter().any(|f| f.module == path) {
            continue;
        }
        synthetic.push((path, doc.blocks.len()));
        doc.blocks.push(Block {
            id: h.module.clone(),
            parent: None,
            kind: Kind::Module,
            item: None,
            title: h.title.clone(),
            doc: h.doc.clone(),
            file: None,
            start_line: 0,
            end_line: 0,
            signature: None,
            defines: Vec::new(),
            order: k,
            start: 0,
            end: 0,
        });
    }
    link_modules(&mut doc, &parsed, &synthetic);
    edges(&mut doc, &parsed);
    doc
}

// ---------------------------------------------------------------------------
// One file

struct ImplRegion {
    open: usize,
    close: usize,
    target: String,
}

struct Group {
    kind: CommentKind,
    start: usize,
    end: usize,
    line: usize,
    lines: Vec<String>,
}

struct FileScan {
    file: usize,
    lang: Lang,
    svelte: bool,
    module: Vec<String>,
    lexed: Lexed,
    /// Brace depth before each token.
    depth: Vec<usize>,
    /// Byte ranges under `#[cfg(test)]`.
    tests: Vec<(usize, usize)>,
    /// Token index ranges of `use` declarations.
    uses: Vec<(usize, usize)>,
    impls: Vec<ImplRegion>,
    /// `mod name;` declarations: name and offset.
    mods: Vec<(String, usize)>,
    /// The blocks of this file (indices into `Doc::blocks`), module block first.
    own: Vec<usize>,
    len: usize,
}

const ITEM_KEYWORDS: &[&str] = &[
    "fn", "struct", "enum", "trait", "impl", "type", "const", "static", "mod", "union",
];

impl FileScan {
    fn new(text: &str, module: Vec<String>, file: usize, lang: Lang, svelte: bool) -> Self {
        let lexed = match (lang, svelte) {
            (Lang::Rust, _) => lex(text),
            (Lang::Ts, false) => lex_ts(text),
            (Lang::Ts, true) => lex_ts(&svelte_code(text)),
        };
        let t = &lexed.tokens;
        let mut depth = Vec::with_capacity(t.len());
        let mut d = 0usize;
        for tok in t {
            depth.push(d);
            if tok.is('{') {
                d += 1;
            } else if tok.is('}') {
                d = d.saturating_sub(1);
            }
        }
        let mut tests = Vec::new();
        let mut uses = Vec::new();
        let mut impls = Vec::new();
        let mut mods = Vec::new();
        let mut k = 0;
        while lang == Lang::Ts && k < t.len() {
            match t[k].ident() {
                // `import … ;` and `export … from '…';` name, they do not use.
                Some("import") if t.get(k + 1).is_none_or(|x| !x.is('(')) => {
                    let end = (k..t.len()).find(|&j| t[j].is(';')).unwrap_or(t.len() - 1);
                    uses.push((k, end));
                    k = end;
                }
                Some("class") if depth[k] <= 1 => {
                    if let (Some(name), Some((open, close))) = (
                        t.get(k + 1).and_then(Token::ident),
                        item_extent(t, k, Lang::Ts),
                    ) {
                        if t[open].is('{') {
                            impls.push(ImplRegion {
                                open: t[open].start,
                                close: t[close].end,
                                target: name.to_owned(),
                            });
                        }
                    }
                }
                _ => {}
            }
            k += 1;
        }
        if lang == Lang::Ts {
            k = t.len();
        }
        while k < t.len() {
            if is_cfg_test(t, k) {
                let start = t[k].start;
                if let Some(h) = skip_attrs(t, k) {
                    if let Some((_, end)) = item_extent(t, h, Lang::Rust) {
                        tests.push((start, t[end].end));
                        k = end + 1;
                        continue;
                    }
                }
            }
            match t[k].ident() {
                Some("use") => {
                    let end = (k..t.len()).find(|&j| t[j].is(';')).unwrap_or(t.len() - 1);
                    uses.push((k, end));
                }
                Some("impl" | "trait") if at_item_position(t, k) => {
                    let target = if t[k].ident() == Some("impl") {
                        impl_target(t, k).map(|(_, ty)| ty)
                    } else {
                        t.get(k + 1).and_then(Token::ident).map(str::to_owned)
                    };
                    if let (Some(target), Some((open, close))) =
                        (target, item_extent(t, k, Lang::Rust))
                    {
                        if t[open].is('{') {
                            impls.push(ImplRegion {
                                open: t[open].start,
                                close: t[close].end,
                                target,
                            });
                        }
                    }
                }
                Some("mod") if t.get(k + 2).is_some_and(|x| x.is(';')) => {
                    if let Some(name) = t.get(k + 1).and_then(Token::ident) {
                        mods.push((name.to_owned(), t[k].start));
                    }
                }
                _ => {}
            }
            k += 1;
        }
        Self {
            file,
            lang,
            svelte,
            module,
            depth,
            tests,
            uses,
            impls,
            mods,
            own: Vec::new(),
            len: text.len(),
            lexed,
        }
    }

    fn in_test(&self, offset: usize) -> bool {
        self.tests.iter().any(|&(s, e)| s <= offset && offset < e)
    }

    fn line_at(&self, offset: usize) -> usize {
        let t = &self.lexed.tokens;
        let k = t.partition_point(|x| x.start < offset);
        t.get(k)
            .map_or_else(|| t.last().map_or(1, |x| x.line), |x| x.line)
    }

    /// The line of the last token before `offset`.
    fn last_line_before(&self, offset: usize) -> usize {
        let t = &self.lexed.tokens;
        let k = t.partition_point(|x| x.start < offset);
        if k == 0 {
            1
        } else {
            t[k - 1].line
        }
    }

    fn enclosing_impl(&self, offset: usize) -> Option<&ImplRegion> {
        self.impls
            .iter()
            .filter(|r| r.open < offset && offset < r.close)
            .min_by_key(|r| r.close - r.open)
    }

    /// Comment groups: consecutive own-line comments of one kind.
    fn groups(&self) -> Vec<Group> {
        let t = &self.lexed.tokens;
        let own_line = |c: &Comment| {
            let k = t.partition_point(|x| x.start < c.start);
            k == 0 || t[k - 1].line != c.line
        };
        let mut out: Vec<Group> = Vec::new();
        let mut prev: Option<(CommentKind, usize, usize)> = None; // kind, line, token count before
        for c in &self.lexed.comments {
            if !own_line(c) {
                prev = None;
                continue;
            }
            let before = t.partition_point(|x| x.start < c.start);
            let joins = prev.is_some_and(|(k, l, b)| k == c.kind && l + 1 == c.line && b == before);
            if joins {
                let g = out.last_mut().expect("a group to join");
                g.lines.push(c.text.clone());
                g.end = c.start + c.text.len() + 3;
            } else {
                out.push(Group {
                    kind: c.kind,
                    start: c.start,
                    end: c.start + c.text.len() + 3,
                    line: c.line,
                    lines: vec![c.text.clone()],
                });
            }
            prev = Some((c.kind, c.line, before));
        }
        out
    }

    fn blocks(&mut self, doc: &mut Doc) {
        let path = doc.files[self.file].path.clone();
        let mut issues = Vec::new();
        let groups = self.groups();
        let t = &self.lexed.tokens;
        let first_tok = t.first().map_or(self.len, |x| x.start);
        let name = self.module.last().cloned().unwrap_or_default();

        // The module block: Rust's `//!` docs, or in TypeScript the first
        // `/** … */` before any code (in a Svelte component, at the top of its
        // first script).
        let ts = self.lang == Lang::Ts;
        let strict = !ts;
        let module_doc = |g: &Group| {
            g.start < first_tok
                && if ts {
                    g.kind == CommentKind::Outer
                } else {
                    g.kind == CommentKind::Inner
                }
        };
        let module_groups: Vec<usize> = if ts {
            groups.iter().position(module_doc).into_iter().collect()
        } else {
            (0..groups.len())
                .filter(|&k| module_doc(&groups[k]))
                .collect()
        };
        let mut mod_lines = Vec::new();
        let mut mod_title = None;
        for &gk in &module_groups {
            let g = &groups[gk];
            let (lines, tags) = split_tags(&g.lines, strict);
            mod_lines.extend(lines);
            mod_lines.push(String::new());
            for (k, (tag, arg)) in tags.into_iter().enumerate() {
                match tag.as_str() {
                    "block" => mod_title = Some(arg),
                    other => issues.push(issue(&path, g.line + k, format!("unknown tag @{other}"))),
                }
            }
        }
        if mod_title.is_none() {
            let how = if ts {
                "`@block <title>` in the file's first `/** … */`"
            } else {
                "`//! @block <title>`"
            };
            issues.push(issue(
                &path,
                1,
                format!("module `{}` has no {how}", self.module.join("::")),
            ));
        }
        let mut found: Vec<Block> = vec![Block {
            id: self.module.join("::"),
            parent: None,
            kind: Kind::Module,
            item: None,
            title: non_empty(mod_title.unwrap_or_default()).unwrap_or_else(|| name.clone()),
            doc: tidy(&mod_lines),
            file: Some(self.file),
            start_line: 1,
            end_line: t.last().map_or(1, |x| x.line),
            signature: None,
            defines: Vec::new(),
            order: 0,
            start: 0,
            end: self.len,
        }];

        // Item and section blocks.
        // A `// @block` line right under an item's `///` docs tags that item;
        // its docs are the item's (rustdoc and schemas never see the tag).
        let docs_above = |g: &Group| {
            groups
                .iter()
                .position(|o| o.kind == CommentKind::Outer && o.line + o.lines.len() == g.line)
        };
        let tags_item = |g: &Group| docs_above(g).is_some();
        let markers: Vec<(usize, usize, bool)> = groups
            .iter()
            .filter(|g| g.kind == CommentKind::Plain && !self.in_test(g.start))
            .filter(|g| !tags_item(g))
            .filter_map(|g| {
                let first = g.lines.first()?.trim();
                let is_block = first == "@block" || first.starts_with("@block ");
                let is_end = first == "@end";
                (is_block || is_end).then(|| (g.start, self.depth_at(g.start), is_end))
            })
            .collect();
        for (gi, g) in groups.iter().enumerate() {
            if self.in_test(g.start) || (ts && module_groups.contains(&gi)) {
                continue;
            }
            match g.kind {
                CommentKind::Outer => {
                    let (lines, tags) = split_tags(&g.lines, strict);
                    let mut title = None;
                    for (tag, arg) in tags {
                        match tag.as_str() {
                            "block" => title = Some(arg),
                            other => {
                                issues.push(issue(&path, g.line, format!("unknown tag @{other}")))
                            }
                        }
                    }
                    let Some(title) = title else { continue };
                    match self.item_block(g, &lines, title, doc) {
                        Ok(b) => found.push(b),
                        Err(m) => issues.push(issue(&path, g.line, m)),
                    }
                }
                CommentKind::Plain => {
                    let first = g.lines[0].trim();
                    if !(first == "@block" || first.starts_with("@block ")) {
                        continue;
                    }
                    let title = first["@block".len()..].trim().to_owned();
                    let (lines, tags) = split_tags(&g.lines[1..], false);
                    for (tag, _) in tags {
                        issues.push(issue(
                            &path,
                            g.line,
                            format!("unknown tag @{tag} in a section"),
                        ));
                    }
                    if title.is_empty() {
                        issues.push(issue(
                            &path,
                            g.line,
                            "a section `// @block` needs a title".into(),
                        ));
                        continue;
                    }
                    if let Some(o) = docs_above(g) {
                        let docs = &groups[o];
                        let (mut lines, _) = split_tags(&docs.lines, true);
                        lines.extend(g.lines[1..].iter().cloned());
                        let span = Group {
                            kind: CommentKind::Plain,
                            start: docs.start,
                            end: g.end,
                            line: docs.line,
                            lines: Vec::new(),
                        };
                        match self.item_block(&span, &lines, title, doc) {
                            Ok(b) => found.push(b),
                            Err(m) => issues.push(issue(&path, g.line, m)),
                        }
                        continue;
                    }
                    let depth = self.depth_at(g.start);
                    let close = self.closing_brace(g.start, depth);
                    let next = markers
                        .iter()
                        .find(|&&(s, d, _)| s > g.start && s < close && d == depth)
                        .copied();
                    let (end, end_line) = match next {
                        Some((s, _, true)) => (s + 1, self.line_at_comment(s)),
                        Some((s, _, false)) => (s, self.last_line_before(s)),
                        None => (close, self.last_line_before(close)),
                    };
                    found.push(Block {
                        id: String::new(),
                        parent: None,
                        kind: Kind::Section,
                        item: None,
                        title,
                        doc: tidy(&lines),
                        file: Some(self.file),
                        start_line: g.line,
                        end_line: end_line.max(g.line),
                        signature: None,
                        defines: Vec::new(),
                        order: g.start,
                        start: g.start,
                        end,
                    });
                }
                CommentKind::Inner => {}
            }
        }
        // `@end` without a section.
        for &(s, d, is_end) in &markers {
            if is_end
                && !found.iter().any(|b| {
                    b.kind == Kind::Section && b.end == s + 1 && self.depth_at(b.start) == d
                })
            {
                issues.push(issue(
                    &path,
                    self.line_at_comment(s),
                    "`// @end` closes no section".into(),
                ));
            }
        }

        // Parents within the file: the smallest block around each.
        let mut order: Vec<usize> = (0..found.len()).collect();
        order.sort_by(|&a, &b| {
            found[a]
                .start
                .cmp(&found[b].start)
                .then(found[b].end.cmp(&found[a].end))
                .then(a.cmp(&b))
        });
        let base = doc.blocks.len();
        let mut local_parent: Vec<Option<usize>> = vec![None; found.len()];
        let mut stack: Vec<usize> = Vec::new();
        for &k in &order {
            while let Some(&top) = stack.last() {
                if found[top].start <= found[k].start && found[k].end <= found[top].end && top != k
                {
                    break;
                }
                stack.pop();
            }
            local_parent[k] = stack.last().copied();
            stack.push(k);
        }
        // Ids, in containment order so a parent's id is known first.
        for &k in &order {
            if let Some(p) = local_parent[k] {
                found[k].parent = Some(base + p);
                if found[k].kind == Kind::Section {
                    let pid = found[p].id.clone();
                    found[k].id = format!("{pid}#{}", slug(&found[k].title));
                }
            }
        }
        let mut seen: HashMap<String, usize> =
            doc.blocks.iter().map(|b| (b.id.clone(), 1)).collect();
        for b in &mut found {
            let n = seen.entry(b.id.clone()).or_insert(0);
            *n += 1;
            if *n > 1 {
                b.id = format!("{}~{n}", b.id);
            }
            if b.doc.trim().is_empty() {
                issues.push(issue(
                    &path,
                    b.start_line,
                    format!("block `{}` has no documentation", b.id),
                ));
            }
        }
        self.own = (base..base + found.len()).collect();
        doc.blocks.extend(found);
        doc.issues.extend(issues);
    }

    fn depth_at(&self, offset: usize) -> usize {
        let t = &self.lexed.tokens;
        let k = t.partition_point(|x| x.start < offset);
        self.depth.get(k).copied().unwrap_or(0)
    }

    /// The offset of the brace closing the block that is open at `offset`
    /// with depth `depth` (the file's end at depth 0).
    fn closing_brace(&self, offset: usize, depth: usize) -> usize {
        if depth == 0 {
            return self.len;
        }
        let t = &self.lexed.tokens;
        let k = t.partition_point(|x| x.start < offset);
        (k..t.len())
            .find(|&j| t[j].is('}') && self.depth[j] == depth)
            .map_or(self.len, |j| t[j].start)
    }

    fn line_at_comment(&self, offset: usize) -> usize {
        self.lexed
            .comments
            .iter()
            .find(|c| c.start == offset)
            .map_or_else(|| self.line_at(offset), |c| c.line)
    }

    fn item_block(
        &self,
        g: &Group,
        lines: &[String],
        title: String,
        doc: &Doc,
    ) -> Result<Block, String> {
        let t = &self.lexed.tokens;
        let k = t.partition_point(|x| x.start < g.end);
        let h = skip_attrs(t, k).ok_or("`@block` in item docs needs an item after them")?;
        let ts = self.lang == Lang::Ts;
        let kw = if ts {
            ts_header(t, h)
                .ok_or("`@block` documents a function, class, method, type or constant")?
        } else {
            header_keyword(t, h).ok_or(
                "`@block` documents a fn, struct, enum, trait, impl, type, const, static or mod",
            )?
        };
        let kwt = &t[kw];
        let word = kwt.ident().unwrap_or_default();
        let keyword = if !ts || TS_KEYWORDS.contains(&word) {
            word.to_owned()
        } else {
            "method".to_owned()
        };
        let (open, end) = item_extent(t, kw, self.lang).ok_or("the item has no end")?;
        let name = if keyword == "impl" {
            let (tr, ty) = impl_target(t, kw).ok_or("cannot read the impl's type")?;
            tr.map_or(ty.clone(), |tr| format!("<{ty} as {tr}>"))
        } else if keyword == "method" {
            word.to_owned()
        } else {
            t[kw + 1..]
                .iter()
                .find(|x| !x.is('*'))
                .and_then(Token::ident)
                .ok_or("the item has no name")?
                .to_owned()
        };
        let mut path = self.module.clone();
        if keyword != "impl" {
            if let Some(r) = self.enclosing_impl(kwt.start) {
                path.push(r.target.clone());
            }
        }
        path.push(name);
        let text = &doc.files[self.file].text;
        let sig_end = if t[open].is('{') {
            t[open].start
        } else {
            t[open].end
        };
        let signature = text[t[h].start..sig_end]
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        Ok(Block {
            id: path.join("::"),
            parent: None,
            kind: Kind::Item,
            item: Some(keyword),
            title: non_empty(title).unwrap_or_else(|| path.last().cloned().unwrap_or_default()),
            doc: tidy(lines),
            file: Some(self.file),
            start_line: g.line,
            end_line: t[end].line,
            signature: Some(signature),
            defines: Vec::new(),
            order: g.start,
            start: g.start,
            end: t[end].end,
        })
    }
}

fn issue(file: &str, line: usize, message: String) -> Issue {
    Issue {
        file: file.to_owned(),
        line,
        message,
    }
}

fn non_empty(s: String) -> Option<String> {
    let s = s.trim().to_owned();
    (!s.is_empty()).then_some(s)
}

/// Split documentation lines into text and `@tag arg` lines (outside code
/// fences). With `all`, every `@word` line is a tag; else only `@block` and
/// `@end` are.
fn split_tags(lines: &[String], all: bool) -> (Vec<String>, Vec<(String, String)>) {
    let mut text = Vec::new();
    let mut tags = Vec::new();
    let mut fence = false;
    for l in lines {
        let s = l.trim();
        if s.starts_with("```") {
            fence = !fence;
        }
        let tag = (!fence)
            .then(|| s.strip_prefix('@'))
            .flatten()
            .filter(|r| r.chars().next().is_some_and(|c| c.is_ascii_alphabetic()));
        match tag {
            Some(r) if all || r.starts_with("block") || r == "end" => {
                let (name, arg) = r.split_once(char::is_whitespace).unwrap_or((r, ""));
                tags.push((name.to_owned(), arg.trim().to_owned()));
            }
            _ => text.push(l.clone()),
        }
    }
    (text, tags)
}

/// Join documentation lines, without leading and trailing blank lines.
fn tidy(lines: &[String]) -> String {
    let s = lines.join("\n");
    s.trim_matches('\n').trim_end().to_owned()
}

fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_owned()
}

// ---------------------------------------------------------------------------
// Token helpers

fn is_cfg_test(t: &[Token], k: usize) -> bool {
    t[k].is('#')
        && t.get(k + 1).is_some_and(|x| x.is('['))
        && t.get(k + 2).and_then(Token::ident) == Some("cfg")
        && t.get(k + 3).is_some_and(|x| x.is('('))
        && t.get(k + 4).and_then(Token::ident) == Some("test")
        && t.get(k + 5).is_some_and(|x| x.is(')'))
}

/// The first token after any outer attributes starting at `k`.
fn skip_attrs(t: &[Token], mut k: usize) -> Option<usize> {
    while k < t.len() && t[k].is('#') && t.get(k + 1).is_some_and(|x| x.is('[')) {
        let mut depth = 0i32;
        let mut j = k + 1;
        loop {
            let x = t.get(j)?;
            if x.is('[') {
                depth += 1;
            } else if x.is(']') {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            j += 1;
        }
        k = j + 1;
    }
    (k < t.len()).then_some(k)
}

/// The item keyword of the header starting at `h` (after visibility and
/// qualifiers).
fn header_keyword(t: &[Token], mut h: usize) -> Option<usize> {
    loop {
        let id = t.get(h)?.ident()?;
        match id {
            "pub" => {
                h += 1;
                if t.get(h).is_some_and(|x| x.is('(')) {
                    while !t.get(h)?.is(')') {
                        h += 1;
                    }
                    h += 1;
                }
            }
            "async" | "unsafe" | "default" => h += 1,
            "extern" => {
                h += 1;
                if t.get(h).is_some_and(|x| x.tok == Tok::Literal) {
                    h += 1;
                }
            }
            "const"
                if t.get(h + 1)
                    .and_then(Token::ident)
                    .is_some_and(|n| matches!(n, "fn" | "unsafe" | "async" | "extern")) =>
            {
                h += 1;
            }
            kw if ITEM_KEYWORDS.contains(&kw) => return Some(h),
            _ => return None,
        }
    }
}

/// TypeScript declaration keywords.
const TS_KEYWORDS: &[&str] = &[
    "function",
    "class",
    "interface",
    "type",
    "const",
    "let",
    "var",
    "enum",
    "namespace",
];

/// TypeScript modifiers before a declaration or class member.
const TS_MODIFIERS: &[&str] = &[
    "export",
    "default",
    "async",
    "declare",
    "abstract",
    "static",
    "public",
    "private",
    "protected",
    "readonly",
    "override",
    "get",
    "set",
    "accessor",
];

/// The declaration keyword of a TypeScript header starting at `h`, or for a
/// class member (`run(…) {`, `windowS = $state(…)`) its name.
fn ts_header(t: &[Token], mut h: usize) -> Option<usize> {
    loop {
        // A private member: `#run(…)`, `async #run(…)`.
        if t.get(h)?.is('#') {
            h += 1;
            continue;
        }
        let id = t.get(h)?.ident()?;
        let next = t.get(h + 1);
        let member = next.is_some_and(|x| x.is('(') || x.is('<') || x.is('=') || x.is(':'));
        if TS_KEYWORDS.contains(&id) {
            return Some(h);
        }
        if TS_MODIFIERS.contains(&id) && !member {
            h += 1;
            continue;
        }
        return member.then_some(h);
    }
}

/// Whether the TypeScript item at `kw` ends at a `;` (a value, a type alias,
/// a class field) rather than with its body.
fn ts_value(t: &[Token], kw: usize) -> bool {
    match t[kw].ident() {
        Some("const" | "let" | "var" | "type") => true,
        Some(w) if TS_KEYWORDS.contains(&w) => false,
        _ => t.get(kw + 1).is_some_and(|x| x.is('=') || x.is(':')),
    }
}

/// The header end (`{` or `;`) and the last token of the item whose
/// keyword is at `kw`.
fn item_extent(t: &[Token], kw: usize, lang: Lang) -> Option<(usize, usize)> {
    let value = match lang {
        Lang::Rust => matches!(t[kw].ident(), Some("const" | "static" | "type")),
        Lang::Ts => ts_value(t, kw),
    };
    let mut depth = 0i32;
    let mut braces = 0i32;
    for j in kw..t.len() {
        let x = &t[j];
        if x.is('(') || x.is('[') {
            depth += 1;
        } else if x.is(')') || x.is(']') {
            depth -= 1;
        } else if x.is('{') {
            if value {
                braces += 1;
            } else if depth == 0 {
                return Some((j, matching_brace(t, j)?));
            }
        } else if x.is('}') && value {
            braces -= 1;
        } else if x.is(';') && depth == 0 && braces == 0 {
            return Some((j, j));
        }
    }
    None
}

fn matching_brace(t: &[Token], open: usize) -> Option<usize> {
    let mut d = 0i32;
    for (j, x) in t.iter().enumerate().skip(open) {
        if x.is('{') {
            d += 1;
        } else if x.is('}') {
            d -= 1;
            if d == 0 {
                return Some(j);
            }
        }
    }
    None
}

/// Whether the keyword at `k` starts an item (not `impl Trait` in a type).
fn at_item_position(t: &[Token], k: usize) -> bool {
    if k == 0 {
        return true;
    }
    let p = &t[k - 1];
    p.is('}')
        || p.is(';')
        || p.is('{')
        || p.is(']')
        || matches!(p.ident(), Some("unsafe" | "pub" | "default"))
        // `pub(crate) trait`
        || (p.is(')') && t[..k].iter().rev().take(4).any(|x| x.ident() == Some("pub")))
}

/// The trait (if any) and the type of the `impl` at `k`.
fn impl_target(t: &[Token], k: usize) -> Option<(Option<String>, String)> {
    let mut j = k + 1;
    // Generic parameters of the impl.
    if t.get(j).is_some_and(|x| x.is('<')) {
        let mut d = 0i32;
        loop {
            let x = t.get(j)?;
            if x.is('<') {
                d += 1;
            } else if x.is('>') && !(j > 0 && t[j - 1].is('-') && t[j - 1].end == x.start) {
                d -= 1;
                if d == 0 {
                    j += 1;
                    break;
                }
            }
            j += 1;
        }
    }
    let mut trait_name = None;
    let mut last: Option<String> = None;
    let mut d = 0i32;
    while let Some(x) = t.get(j) {
        if x.is('{') || x.is(';') || (d == 0 && x.ident() == Some("where")) {
            break;
        }
        if x.is('<') {
            d += 1;
        } else if x.is('>') {
            d -= 1;
        } else if d == 0 {
            match x.ident() {
                Some("for") => trait_name = last.take(),
                Some("dyn" | "mut" | "const") | None => {}
                Some(id) => last = Some(id.to_owned()),
            }
        }
        j += 1;
    }
    last.map(|ty| (trait_name, ty))
}

// ---------------------------------------------------------------------------
// Module tree and edges

/// Hang every module under its parent: the smallest block of the parent's
/// file around its `mod` declaration, else the parent's block (a file or a
/// heading). Modules without a declaration keep their file order.
fn link_modules(doc: &mut Doc, files: &[FileScan], synthetic: &[(Vec<String>, usize)]) {
    let mut modules: Vec<(Vec<String>, usize, Option<&FileScan>)> = files
        .iter()
        .map(|f| (f.module.clone(), f.own[0], Some(f)))
        .collect();
    modules.extend(synthetic.iter().map(|(p, b)| (p.clone(), *b, None)));
    for (k, (path, me, _)) in modules.iter().enumerate() {
        if path.len() < 2 {
            continue;
        }
        let parent_path = &path[..path.len() - 1];
        let name = &path[path.len() - 1];
        let Some((_, parent_block, parent_file)) = modules.iter().find(|m| m.0 == parent_path)
        else {
            continue;
        };
        let decl =
            parent_file.and_then(|pf| pf.mods.iter().find(|(n, _)| n == name).map(|&(_, off)| off));
        let parent = match (decl, parent_file) {
            // A section of the parent's file can group modules.
            (Some(off), Some(pf)) => pf
                .own
                .iter()
                .copied()
                .filter(|&b| doc.blocks[b].start <= off && off < doc.blocks[b].end)
                .min_by_key(|&b| doc.blocks[b].end - doc.blocks[b].start)
                .unwrap_or(*parent_block),
            _ => *parent_block,
        };
        doc.blocks[*me].parent = Some(parent);
        doc.blocks[*me].order = match decl {
            Some(off) => off,
            // Headings in the order given, then files in path order.
            None if doc.blocks[*me].file.is_none() => doc.blocks[*me].order,
            None => 1_000_000 + k,
        };
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DefKind {
    Type,
    Fn,
    Value,
    Module,
}

struct Def {
    name: String,
    owner: Option<String>,
    kind: DefKind,
    block: usize,
    file: usize,
}

/// Method names that the standard library also has: a `.name(` call is more
/// likely the standard one than a project method of the same name.
const STD_METHODS: &[&str] = &[
    "new",
    "len",
    "is_empty",
    "iter",
    "iter_mut",
    "get",
    "get_mut",
    "push",
    "pop",
    "insert",
    "remove",
    "clone",
    "default",
    "fmt",
    "from",
    "into",
    "min",
    "max",
    "abs",
    "map",
    "take",
    "extend",
    "contains",
    "eq",
    "ne",
    "cmp",
    "partial_cmp",
    "hash",
    "next",
    "as_ref",
    "as_slice",
    "to_string",
    "sum",
    "sqrt",
    "clear",
    "resize",
    "fill",
    "sort",
    "sort_by",
    "retain",
    "first",
    "last",
    "split",
    "join",
    "into_iter",
    "entry",
    "unwrap",
    "expect",
    "ok",
    "err",
    "and_then",
    "map_err",
    "unwrap_or",
    "drain",
    "truncate",
    "swap",
    "chain",
    "zip",
    "rev",
    "filter",
    "count",
    "position",
    "any",
    "all",
    "find",
    "fold",
    "enumerate",
    "collect",
    "keys",
    "values",
    "trim",
    "parse",
    "serialize",
    "deserialize",
    "copied",
    "cloned",
    "replace",
    "set",
    "with",
    "write",
    "read",
    "is_some",
    "is_none",
    "len_utf8",
    "neg",
    "add",
    "sub",
    "mul",
    "div",
    "index",
    "drop",
    // JavaScript built-ins.
    "then",
    "catch",
    "finally",
    "forEach",
    "slice",
    "splice",
    "includes",
    "indexOf",
    "toFixed",
    "has",
    "delete",
    "entries",
    "reduce",
    "some",
    "every",
    "concat",
    "flat",
    "flatMap",
    "subarray",
    "test",
    "exec",
    "match",
    "replaceAll",
    "padStart",
    "padEnd",
    "toUpperCase",
    "toLowerCase",
    "startsWith",
    "endsWith",
    "at",
    "trimEnd",
    "trimStart",
    "getContext",
    "querySelector",
    "addEventListener",
    "removeEventListener",
    "preventDefault",
    "stopPropagation",
    "toString",
    "valueOf",
    "focus",
    "blur",
    "abort",
    "json",
    "text",
];

fn owner_of(doc: &Doc, fs: &FileScan, offset: usize) -> usize {
    fs.own
        .iter()
        .copied()
        .filter(|&b| doc.blocks[b].start <= offset && offset < doc.blocks[b].end)
        .min_by_key(|&b| doc.blocks[b].end - doc.blocks[b].start)
        .unwrap_or(fs.own[0])
}

/// The definitions of a TypeScript file: its module-level declarations,
/// the methods of its classes, and for a Svelte component the component.
fn ts_defs(doc: &Doc, fs: &FileScan, defs: &mut Vec<Def>) {
    let t = &fs.lexed.tokens;
    if fs.svelte {
        defs.push(Def {
            name: fs.module.last().cloned().unwrap_or_default(),
            owner: None,
            kind: DefKind::Type,
            block: fs.own[0],
            file: fs.file,
        });
    }
    let in_use = |k: usize| fs.uses.iter().any(|&(s, e)| s <= k && k <= e);
    for k in 0..t.len() {
        let Some(word) = t[k].ident() else { continue };
        if in_use(k) || (k > 0 && t[k - 1].is('.')) {
            continue;
        }
        let class = fs.enclosing_impl(t[k].start);
        if TS_KEYWORDS.contains(&word) && class.is_none() && fs.depth[k] == 0 {
            let kind = match word {
                "class" | "interface" | "type" | "enum" => DefKind::Type,
                "function" => DefKind::Fn,
                _ => DefKind::Value,
            };
            let Some(name) = t[k + 1..]
                .iter()
                .find(|x| !x.is('*'))
                .and_then(Token::ident)
            else {
                continue;
            };
            defs.push(Def {
                name: name.to_owned(),
                owner: None,
                kind,
                block: owner_of(doc, fs, t[k].start),
                file: fs.file,
            });
        } else if let Some(c) = class {
            // A method: a name and `(` at the class body's depth, after a
            // member boundary or a modifier.
            let body = fs
                .depth
                .get(t.partition_point(|x| x.start <= c.open))
                .copied();
            // After a member boundary or a modifier (`#name` for a private one).
            let p = if k > 0 && t[k - 1].is('#') { k - 1 } else { k };
            let starts_member = p > 0
                && (t[p - 1].is('{')
                    || t[p - 1].is('}')
                    || t[p - 1].is(';')
                    || t[p - 1].ident().is_some_and(|q| TS_MODIFIERS.contains(&q)));
            if Some(fs.depth[k]) == body
                && starts_member
                && t.get(k + 1).is_some_and(|x| x.is('(') || x.is('<'))
                && !matches!(
                    word,
                    "if" | "for" | "while" | "switch" | "return" | "constructor"
                )
            {
                defs.push(Def {
                    name: word.to_owned(),
                    owner: Some(c.target.clone()),
                    kind: DefKind::Fn,
                    block: owner_of(doc, fs, t[k].start),
                    file: fs.file,
                });
            }
        }
    }
}

fn edges(doc: &mut Doc, files: &[FileScan]) {
    // Definitions.
    let mut defs: Vec<Def> = Vec::new();
    for fs in files {
        let t = &fs.lexed.tokens;
        defs.push(Def {
            name: fs.module.last().cloned().unwrap_or_default(),
            owner: None,
            kind: DefKind::Module,
            block: fs.own[0],
            file: fs.file,
        });
        if fs.lang == Lang::Ts {
            ts_defs(doc, fs, &mut defs);
            continue;
        }
        for k in 0..t.len() {
            let Some(kw) = t[k].ident() else { continue };
            if fs.in_test(t[k].start) {
                continue;
            }
            let kind = match kw {
                "struct" | "enum" | "trait" | "union" => DefKind::Type,
                "type" => DefKind::Type,
                "fn" => DefKind::Fn,
                "const" | "static" => DefKind::Value,
                _ => continue,
            };
            if k > 0
                && (t[k - 1].is('<') || t[k - 1].is(',') || t[k - 1].is('*') || t[k - 1].is('&'))
            {
                continue;
            }
            let Some(name) = t.get(k + 1).and_then(Token::ident) else {
                continue;
            };
            if name == "fn" || name == "mut" {
                continue;
            }
            let owner = fs.enclosing_impl(t[k].start).map(|r| r.target.clone());
            if kind == DefKind::Type && kw == "type" && owner.is_some() {
                continue; // associated types
            }
            let block = owner_of(doc, fs, t[k].start);
            defs.push(Def {
                name: name.to_owned(),
                owner,
                kind,
                block,
                file: fs.file,
            });
        }
    }
    let mut by_name: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, d) in defs.iter().enumerate() {
        by_name.entry(d.name.as_str()).or_default().push(i);
    }
    let types: BTreeSet<&str> = defs
        .iter()
        .filter(|d| d.kind == DefKind::Type)
        .map(|d| d.name.as_str())
        .collect();
    let modules: BTreeSet<&str> = defs
        .iter()
        .filter(|d| d.kind == DefKind::Module)
        .map(|d| d.name.as_str())
        .collect();

    // The names each block defines itself.
    let mut defines: Vec<BTreeSet<String>> = vec![BTreeSet::new(); doc.blocks.len()];
    for d in &defs {
        if d.kind != DefKind::Module {
            defines[d.block].insert(match &d.owner {
                Some(o) if d.kind == DefKind::Fn => format!("{o}::{}", d.name),
                _ => d.name.clone(),
            });
        }
    }

    // References.
    let mut found: BTreeMap<(usize, usize), (usize, BTreeSet<String>)> = BTreeMap::new();
    for fs in files {
        let t = &fs.lexed.tokens;
        let mut skip_use = fs.uses.iter().peekable();
        for k in 0..t.len() {
            while skip_use.peek().is_some_and(|&&(_, e)| e < k) {
                skip_use.next();
            }
            if skip_use.peek().is_some_and(|&&(s, e)| s <= k && k <= e) {
                continue;
            }
            let Some(name) = t[k].ident() else { continue };
            let Some(cands) = by_name.get(name) else {
                continue;
            };
            if fs.in_test(t[k].start) {
                continue;
            }
            // The name of a definition is not a use.
            if k > 0
                && t[k - 1].ident().is_some_and(|p| {
                    matches!(
                        p,
                        "fn" | "struct"
                            | "enum"
                            | "trait"
                            | "type"
                            | "const"
                            | "static"
                            | "mod"
                            | "union"
                            | "function"
                            | "class"
                            | "interface"
                            | "let"
                            | "var"
                    )
                })
            {
                continue;
            }
            let next = t.get(k + 1);
            let call = next.is_some_and(|x| x.is('('));
            let path_next =
                next.is_some_and(|x| x.is(':')) && t.get(k + 2).is_some_and(|x| x.is(':'));
            let turbofish = path_next && t.get(k + 3).is_some_and(|x| x.is('<'));
            let qualifier =
                (k >= 3 && t[k - 1].is(':') && t[k - 2].is(':') && t[k - 2].end == t[k - 1].start)
                    .then(|| t[k - 3].ident())
                    .flatten();
            // `x.run(`, and a private `this.#run(`.
            let dot =
                k > 0 && (t[k - 1].is('.') || (t[k - 1].is('#') && k > 1 && t[k - 2].is('.')));
            let method = dot && (call || turbofish);
            let upper = name.chars().next().is_some_and(char::is_uppercase);
            let filter = |d: &Def| -> bool {
                // Names match within one language only: a TypeScript `append`
                // is not the Rust one.
                if doc.files[d.file].lang != fs.lang {
                    return false;
                }
                if let Some(q) = qualifier {
                    return match q {
                        "Self" => {
                            let me = fs.enclosing_impl(t[k].start).map(|r| r.target.as_str());
                            d.owner.as_deref() == me && me.is_some()
                        }
                        "self" | "super" | "crate" => d.owner.is_none(),
                        q if types.contains(q) => d.owner.as_deref() == Some(q),
                        q if modules.contains(q) => {
                            d.owner.is_none()
                                && (d.kind == DefKind::Module
                                    || files[d.file].module.last().map(String::as_str) == Some(q))
                        }
                        _ => false,
                    };
                }
                if method {
                    return d.kind == DefKind::Fn
                        && d.owner.is_some()
                        && !STD_METHODS.contains(&name);
                }
                if k > 0 && t[k - 1].is('.') {
                    return false; // a field
                }
                if d.owner.is_some() {
                    return false;
                }
                if path_next {
                    return matches!(d.kind, DefKind::Module | DefKind::Type);
                }
                if call {
                    return matches!(d.kind, DefKind::Fn | DefKind::Type)
                        || (fs.lang == Lang::Ts && d.kind == DefKind::Value);
                }
                upper && matches!(d.kind, DefKind::Type | DefKind::Value)
            };
            // A module path (`init::build`) counts by its item, not its module.
            if path_next
                && t.get(k + 3)
                    .and_then(Token::ident)
                    .is_some_and(|n| by_name.contains_key(n))
            {
                continue;
            }
            let from = owner_of(doc, fs, t[k].start);
            let mut hits: Vec<&Def> = cands
                .iter()
                .map(|&i| &defs[i])
                .filter(|d| filter(d))
                .collect();
            if hits.len() > 1 {
                let local: Vec<&Def> = hits.iter().copied().filter(|d| d.file == fs.file).collect();
                if !local.is_empty() {
                    hits = local;
                }
            }
            let mut targets: Vec<usize> = hits.iter().map(|d| d.block).collect();
            targets.sort_unstable();
            targets.dedup();
            if targets.len() != 1 {
                continue;
            }
            let d = hits[0];
            let to = targets[0];
            if to == from {
                continue;
            }
            let label = match (&d.owner, d.kind) {
                (Some(o), DefKind::Fn) => format!("{o}::{}", d.name),
                (_, DefKind::Module) => format!("{}::", d.name),
                _ => d.name.clone(),
            };
            let e = found.entry((from, to)).or_default();
            e.0 += 1;
            e.1.insert(label);
        }
    }
    for (b, names) in defines.into_iter().enumerate() {
        doc.blocks[b].defines = names.into_iter().collect();
    }
    doc.edges = found
        .into_iter()
        .map(|((from, to), (count, names))| Edge {
            from,
            to,
            count,
            names: names.into_iter().collect(),
        })
        .collect();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(files: &[(&str, &str)]) -> Doc {
        build(
            "test",
            files
                .iter()
                .map(|(m, src)| {
                    let module: Vec<String> = m.split("::").map(str::to_owned).collect();
                    (
                        format!("{}.rs", m.replace("::", "/")),
                        module,
                        (*src).to_owned(),
                    )
                })
                .collect(),
        )
    }

    const ROOT: &str = "//! The root.\n//!\n//! @block Root\n\nmod a;\nmod b;\n";
    const A: &str = r"//! Module a.
//! @block Alpha

/// Holds things.
/// @block The holder
pub struct Holder { x: f64 }

impl Holder {
    /// Runs it.
    // @block Run
    pub fn run(&self) -> f64 {
        // @block Prepare
        // Get ready.
        let y = 1.0;
        // @block Finish
        // Done.
        crate::b::helper(y)
    }
}
";
    const B: &str = r"//! Module b.
//! @block Beta

pub fn helper(v: f64) -> f64 { v }

fn uses_holder(h: &super::a::Holder) -> f64 { h.run() }

#[cfg(test)]
mod tests {
    fn t() { super::super::a::Holder { x: 1.0 }; }
}
";

    #[test]
    fn blocks_nest_by_where_they_are() {
        let d = doc(&[("r", ROOT), ("r::a", A), ("r::b", B)]);
        assert!(d.issues.is_empty(), "{:?}", d.issues);
        let ids: Vec<&str> = d.blocks.iter().map(|b| b.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "r",
                "r::a",
                "r::a::Holder",
                "r::a::Holder::run",
                "r::a::Holder::run#prepare",
                "r::a::Holder::run#finish",
                "r::b"
            ]
        );
        let parent = |id: &str| {
            let b = d.find(id).unwrap();
            b.parent.map(|p| d.blocks[p].id.as_str())
        };
        assert_eq!(parent("r::a"), Some("r"));
        assert_eq!(parent("r::a::Holder::run"), Some("r::a"));
        assert_eq!(
            parent("r::a::Holder::run#finish"),
            Some("r::a::Holder::run")
        );
        let finish = d.find("r::a::Holder::run#finish").unwrap();
        assert_eq!(finish.doc, "Done.");
        // The plain form under the docs: the docs are the item's, tag-free.
        assert_eq!(d.find("r::a::Holder::run").unwrap().doc, "Runs it.");
        assert_eq!(d.find("r::a::Holder").unwrap().doc, "Holds things.");
        assert_eq!((finish.start_line, finish.end_line), (15, 17));
        assert_eq!(
            d.find("r::a::Holder").unwrap().signature.as_deref(),
            Some("pub struct Holder")
        );
    }

    #[test]
    fn edges_come_from_the_code() {
        let d = doc(&[("r", ROOT), ("r::a", A), ("r::b", B)]);
        let edge = |from: &str, to: &str| {
            let f = d.blocks.iter().position(|b| b.id == from).unwrap();
            let t = d.blocks.iter().position(|b| b.id == to).unwrap();
            d.edges
                .iter()
                .find(|e| e.from == f && e.to == t)
                .map(|e| e.names.clone())
        };
        assert_eq!(
            edge("r::a::Holder::run#finish", "r::b"),
            Some(vec!["helper".to_owned()])
        );
        assert_eq!(
            edge("r::b", "r::a::Holder::run"),
            Some(vec!["Holder::run".to_owned()])
        );
        assert!(edge("r::b", "r::a::Holder").is_some());
        // Test code makes no edge (b's test builds a Holder, b's code only
        // names it in a signature, which is an edge already).
        assert_eq!(
            d.edges
                .iter()
                .filter(|e| d.blocks[e.from].id == "r::b")
                .count(),
            2
        );
    }

    #[test]
    fn missing_and_unknown_annotations_are_issues() {
        let d = doc(&[
            ("r", "//! No tag here.\n\nmod a;\n"),
            (
                "r::a",
                "//! A.\n//! @block A\n//! @owner me\n\n// @end\n/// @block\nlet x = 1;\n",
            ),
        ]);
        let msgs: Vec<String> = d.issues.iter().map(ToString::to_string).collect();
        assert!(
            msgs.iter().any(|m| m.contains("has no `//! @block")),
            "{msgs:?}"
        );
        assert!(
            msgs.iter().any(|m| m.contains("unknown tag @owner")),
            "{msgs:?}"
        );
        assert!(
            msgs.iter().any(|m| m.contains("closes no section")),
            "{msgs:?}"
        );
        assert!(
            msgs.iter().any(|m| m.contains("documents a fn")),
            "{msgs:?}"
        );
    }

    #[test]
    fn a_section_can_end_early() {
        let src = "//! M.\n//! @block M\nfn f() {\n    // @block One\n    // First.\n    let a = 1;\n    // @end\n    let b = 2;\n}\n";
        let d = doc(&[("m", src)]);
        assert!(d.issues.is_empty(), "{:?}", d.issues);
        let one = d.find("m#one").unwrap();
        assert_eq!((one.start_line, one.end_line), (4, 7));
    }

    #[test]
    fn typescript_and_svelte_blocks_and_edges() {
        let logic = r#"/** Pacing of the live view.
 * @block Live logic
 */
import { thing } from './x';

/** The time to aim for.
 * @param wall seconds
 * @block Pacing target
 */
export function paceTarget(wall: number): number {
	// @block Clamp
	// Never behind the start.
	const s = `t ${wall} {`;
	return Math.max(0, wall / 2);
}

export class LiveView {
	/** Start a session.
	 * @block Start
	 */
	start(dt: number) {
		return paceTarget(dt);
	}
}
"#;
        let ui = "<script lang=\"ts\">\n\t/** The workbench.\n\t * @block Workbench\n\t */\n\timport { LiveView } from './logic';\n\tconst live = new LiveView();\n\tfunction go() { live.start(1); }\n</script>\n\n<p>Don't {go()}</p>\n<ZoomChart />\n";
        let chart = "<script lang=\"ts\">\n\t/** A chart.\n\t * @block Zoom chart\n\t */\n\tlet { x } = $props();\n</script>\n<svg></svg>\n";
        let d = build_with(
            "t",
            vec![
                (
                    "app/logic.ts".into(),
                    vec!["app".into(), "logic".into()],
                    logic.into(),
                ),
                (
                    "app/Workbench.svelte".into(),
                    vec!["app".into(), "Workbench".into()],
                    ui.into(),
                ),
                (
                    "app/ZoomChart.svelte".into(),
                    vec!["app".into(), "ZoomChart".into()],
                    chart.into(),
                ),
            ],
            &[Heading {
                module: "app".into(),
                title: "App".into(),
                doc: "The app.".into(),
            }],
        );
        assert!(d.issues.is_empty(), "{:?}", d.issues);
        let ids: Vec<&str> = d.blocks.iter().map(|b| b.id.as_str()).collect();
        for id in [
            "app",
            "app::logic",
            "app::logic::paceTarget",
            "app::logic::paceTarget#clamp",
            "app::logic::LiveView::start",
            "app::Workbench",
            "app::ZoomChart",
        ] {
            assert!(ids.contains(&id), "{id} missing from {ids:?}");
        }
        let pace = d.find("app::logic::paceTarget").unwrap();
        assert_eq!(pace.doc, "The time to aim for.\n@param wall seconds");
        assert_eq!(pace.item.as_deref(), Some("function"));
        assert_eq!(d.find("app::logic").unwrap().title, "Live logic");
        let edge = |from: &str, to: &str| {
            let f = d.blocks.iter().position(|b| b.id == from).unwrap();
            let t = d.blocks.iter().position(|b| b.id == to).unwrap();
            d.edges.iter().any(|e| e.from == f && e.to == t)
        };
        // A method call, a class, a component used in markup, a function call.
        assert!(edge("app::Workbench", "app::logic::LiveView::start"));
        assert!(edge("app::Workbench", "app::ZoomChart"));
        assert!(edge(
            "app::logic::LiveView::start",
            "app::logic::paceTarget"
        ));
        let parent = |id: &str| d.find(id).unwrap().parent.map(|p| d.blocks[p].id.as_str());
        assert_eq!(parent("app::logic"), Some("app"));
        assert_eq!(parent("app::Workbench"), Some("app"));
    }
}
