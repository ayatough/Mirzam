// Strips the comments out of the CSS and JavaScript that ships inside every
// deck, before the crate embeds it. The stripper itself lives in
// `src/theme/strip.rs` so that the crate's own tests can hold it to its
// promises; it is included here rather than depended on, because a build
// script cannot use the crate it is building.
//
// The source files are never touched. What changes is only what a reader of a
// deck downloads: about 90 KB less of it.

include!("src/theme/strip.rs");

use std::path::Path;

/// Every asset shipped verbatim inside a deck, and the language it is in.
const ASSETS: &[(&str, Lang)] = &[
    ("base.css", Lang::Css),
    ("print.css", Lang::Css),
    ("handout.css", Lang::Css),
    ("viewer.js", Lang::Js),
    ("anim.js", Lang::Js),
    ("annot.js", Lang::Js),
    ("effects.js", Lang::Js),
    ("fit.js", Lang::Js),
    ("presenter.js", Lang::Js),
    ("themes/mirzam.css", Lang::Css),
    ("themes/nord.css", Lang::Css),
    ("themes/solarized.css", Lang::Css),
    ("themes/vscode.css", Lang::Css),
    ("themes/wuwei.css", Lang::Css),
];

fn main() {
    let out = std::env::var("OUT_DIR").expect("cargo sets OUT_DIR");
    let out = Path::new(&out);
    std::fs::create_dir_all(out.join("themes")).expect("create the themes directory");

    for (name, lang) in ASSETS {
        let src = Path::new("src/theme").join(name);
        println!("cargo:rerun-if-changed={}", src.display());
        let text = std::fs::read_to_string(&src)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", src.display()));
        std::fs::write(out.join(name), strip_comments(&text, *lang))
            .unwrap_or_else(|e| panic!("cannot write the stripped {name}: {e}"));
    }
    println!("cargo:rerun-if-changed=src/theme/strip.rs");
}
