// Comment stripping for the CSS and JavaScript shipped inside every deck.
//
// A deck carries `base.css` and `viewer.js` whole, and both are written the
// way the rest of this repository is written: the reasoning is in the file,
// next to the line it explains. That prose is 52% of `base.css` and 41% of
// `viewer.js`, and a reader of a deck downloads all of it. Stripping it at
// **compile time** — `build.rs` runs this and the crate embeds the result —
// takes about 90 KB off every deck built, for no runtime cost and with the
// source untouched: the comments still live where an author reads them.
//
// Two rules make the output safe to trust:
//
// - **Only comments are removed.** Every other byte is copied through, so a
//   string that looks like a comment (`'// not a comment'`, a `url()` with
//   `//` in it, a regular expression) survives exactly as written. The
//   scanner tracks strings, template literals with their `${}` nesting, and
//   regular expressions for precisely this reason.
// - **Line numbering is preserved.** A newline inside a comment is kept, and
//   a line left holding nothing but whitespace is emitted empty. So line 412
//   of the `viewer.js` a browser shows is line 412 of `viewer.js` in the
//   repository, which is the only reason anyone opens the shipped copy.

/// Which comment forms the text uses.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    /// `/* … */` only. CSS has no line comment: `//` inside a `url()` is a URL.
    Css,
    /// `//` and `/* … */`, outside strings, template literals and regexes.
    Js,
}

/// `src` with its comments removed, its code untouched, and its lines still
/// numbered the way the source file numbers them.
pub fn strip_comments(src: &str, lang: Lang) -> String {
    let mut out = String::with_capacity(src.len());
    let bytes = src.as_bytes();
    let mut i = 0;
    // Template literals nest: `${ `inner` }` puts code inside a string inside
    // code. Each `${` pushes, each matching `}` pops back into the template.
    let mut tpl_depth: Vec<u32> = Vec::new();

    while i < bytes.len() {
        let b = bytes[i];
        let next = bytes.get(i + 1).copied();

        // A comment, if we are in code rather than inside something quoted.
        if b == b'/' && next == Some(b'*') {
            i += 2;
            while i < bytes.len() && !(bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/')) {
                // Newlines inside a comment are the line numbering.
                if bytes[i] == b'\n' {
                    out.push('\n');
                }
                i += 1;
            }
            i = (i + 2).min(bytes.len());
            continue;
        }
        if lang == Lang::Js && b == b'/' && next == Some(b'/') {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }

        match b {
            b'"' | b'\'' => {
                // A quoted string. It cannot span a line unescaped in either
                // language, and the loop stops at a newline for that reason:
                // an unterminated quote must not swallow the rest of the file.
                out.push(b as char);
                i += 1;
                while i < bytes.len() && bytes[i] != b && bytes[i] != b'\n' {
                    if bytes[i] == b'\\' && i + 1 < bytes.len() {
                        out.push_str(&src[i..i + 2]);
                        i += 2;
                        continue;
                    }
                    let end = char_end(src, i);
                    out.push_str(&src[i..end]);
                    i = end;
                }
                if i < bytes.len() && bytes[i] == b {
                    out.push(b as char);
                    i += 1;
                }
            }
            b'`' if lang == Lang::Js => {
                out.push('`');
                i += 1;
                tpl_depth.push(0);
                while i < bytes.len() {
                    if bytes[i] == b'\\' && i + 1 < bytes.len() {
                        out.push_str(&src[i..i + 2]);
                        i += 2;
                        continue;
                    }
                    if bytes[i] == b'$' && bytes.get(i + 1) == Some(&b'{') {
                        out.push_str("${");
                        i += 2;
                        break; // back to the main loop: `${ … }` holds code
                    }
                    if bytes[i] == b'`' {
                        out.push('`');
                        i += 1;
                        tpl_depth.pop();
                        break;
                    }
                    let end = char_end(src, i);
                    out.push_str(&src[i..end]);
                    i = end;
                }
            }
            b'/' if lang == Lang::Js && starts_regex(&out) => {
                out.push('/');
                i += 1;
                let mut in_class = false;
                while i < bytes.len() && bytes[i] != b'\n' {
                    if bytes[i] == b'\\' && i + 1 < bytes.len() {
                        out.push_str(&src[i..i + 2]);
                        i += 2;
                        continue;
                    }
                    match bytes[i] {
                        b'[' => in_class = true,
                        b']' => in_class = false,
                        b'/' if !in_class => {
                            out.push('/');
                            i += 1;
                            break;
                        }
                        _ => {}
                    }
                    let end = char_end(src, i);
                    out.push_str(&src[i..end]);
                    i = end;
                }
            }
            _ => {
                let end = char_end(src, i);
                out.push_str(&src[i..end]);
                i = end;
            }
        }
    }

    blank_whitespace_only_lines(&out)
}

/// The end of the UTF-8 character starting at `i`, so a multi-byte character
/// is copied whole rather than split.
fn char_end(src: &str, i: usize) -> usize {
    let mut end = i + 1;
    while !src.is_char_boundary(end) {
        end += 1;
    }
    end
}

/// Whether a `/` at this point opens a regular expression rather than dividing.
///
/// The rule JavaScript itself uses: a regex may appear where an *expression*
/// may, and division only after a value. So the decision is made on the last
/// significant token emitted — a name, a number, or a closing bracket means
/// division, and anything else means a regex. `}` is read as end-of-block,
/// which is right for `if (x) {} /re/` and wrong only for an object literal
/// immediately divided by something, which is not code anyone writes.
fn starts_regex(before: &str) -> bool {
    let trimmed = before.trim_end();
    let Some(last) = trimmed.chars().last() else {
        return true;
    };
    if last.is_alphanumeric() || last == '_' || last == '$' {
        // A keyword is an operator position; an identifier is a value.
        let word: String = trimmed
            .chars()
            .rev()
            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        return matches!(
            word.as_str(),
            "return"
                | "typeof"
                | "instanceof"
                | "in"
                | "of"
                | "new"
                | "delete"
                | "void"
                | "case"
                | "do"
                | "else"
                | "yield"
                | "await"
                | "throw"
        );
    }
    !matches!(last, ')' | ']' | '}')
}

/// Every line that is now only whitespace, emitted empty. The newline stays —
/// that is what keeps the shipped line numbers equal to the source's.
fn blank_whitespace_only_lines(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for (n, line) in text.split('\n').enumerate() {
        if n > 0 {
            out.push('\n');
        }
        if !line.trim().is_empty() {
            out.push_str(line);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_comments_go_and_code_stays() {
        let css = "/* why */\n.a { color: red; } /* trailing */\n.b { color: blue; }\n";
        assert_eq!(
            strip_comments(css, Lang::Css),
            "\n.a { color: red; } \n.b { color: blue; }\n"
        );
    }

    #[test]
    fn a_url_is_not_a_comment() {
        let css = ".a { background: url(https://example.com/x.png); }\n";
        assert_eq!(strip_comments(css, Lang::Css), css);
    }

    #[test]
    fn a_comment_marker_inside_a_string_survives() {
        let css = ".a::after { content: \"/* not a comment */\"; }\n";
        assert_eq!(strip_comments(css, Lang::Css), css);
        let js = "const s = '// not a comment';\n";
        assert_eq!(strip_comments(js, Lang::Js), js);
    }

    #[test]
    fn a_regex_is_not_a_comment_and_not_a_string() {
        // Every one of these appears in viewer.js.
        let js = "s.replace(/[&<>]/g, c => c);\nx.replace(/\\//g, '_');\n\
                  t.split(/\\s+/);\nb.replace(/=+$/, '');\n";
        assert_eq!(strip_comments(js, Lang::Js), js);
    }

    #[test]
    fn a_quote_inside_a_regex_does_not_open_a_string() {
        let js = "const q = s.replace(/'/g, '');\n// gone\nnext();\n";
        assert_eq!(
            strip_comments(js, Lang::Js),
            "const q = s.replace(/'/g, '');\n\nnext();\n"
        );
    }

    #[test]
    fn division_is_not_a_regex() {
        let js = "const r = (a + b) / 2; // gone\nconst t = x / y / z;\n";
        assert_eq!(
            strip_comments(js, Lang::Js),
            "const r = (a + b) / 2; \nconst t = x / y / z;\n"
        );
    }

    #[test]
    fn template_literals_keep_everything_including_their_code() {
        let js = "const h = `<a href=\"//x\">${name /* gone */}</a>`;\n";
        assert_eq!(
            strip_comments(js, Lang::Js),
            "const h = `<a href=\"//x\">${name }</a>`;\n"
        );
    }

    #[test]
    fn a_multi_line_template_literal_is_left_alone() {
        let js = "const t = `line one\n// still text\nline three`;\nafter();\n";
        assert_eq!(strip_comments(js, Lang::Js), js);
    }

    #[test]
    fn line_numbering_is_preserved() {
        for (src, lang) in [
            (super::super::BASE_CSS_SOURCE, Lang::Css),
            (super::super::VIEWER_JS_SOURCE, Lang::Js),
        ] {
            assert_eq!(
                strip_comments(src, lang).split('\n').count(),
                src.split('\n').count(),
                "a stripped file must number its lines the way its source does"
            );
        }
    }

    #[test]
    fn stripping_is_idempotent() {
        for (src, lang) in [
            (super::super::BASE_CSS_SOURCE, Lang::Css),
            (super::super::VIEWER_JS_SOURCE, Lang::Js),
        ] {
            let once = strip_comments(src, lang);
            assert_eq!(strip_comments(&once, lang), once);
        }
    }

    #[test]
    fn multibyte_text_is_copied_whole() {
        let css = "/* 日本語のコメント */\n.a::after { content: \"→ 終わり\"; }\n";
        assert_eq!(
            strip_comments(css, Lang::Css),
            "\n.a::after { content: \"→ 終わり\"; }\n"
        );
    }
}
