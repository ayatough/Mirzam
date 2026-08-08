//! Assembles the built-in theme's CSS and the viewer runtime JS from the
//! files in this directory. Decks override the result with frontmatter `css:`.
//!
//! - `themes/default.css` — the theme tokens (`:root` custom properties)
//! - `base.css` — layout, typography, panes; everything that is not a token
//! - `print.css` — overrides applied for PDF export
//! - `viewer.js` — the runtime shipped inside every deck
//!
//! `DEFAULT_CSS` is the token file followed by the base file, byte for byte
//! what used to be one file; splitting it must not change a single rendered
//! byte, which is what `crates/mirzam-cli/tests/golden.rs` checks.

pub const DEFAULT_CSS: &str = concat!(
    "\n",
    include_str!("themes/default.css"),
    include_str!("base.css")
);

pub const VIEWER_JS: &str = concat!("\n", include_str!("viewer.js"));

/// Print overrides applied after DEFAULT_CSS.
/// Slide dimensions and the `@page` size are appended by `assemble_print_page`.
pub const PRINT_CSS: &str = concat!("\n", include_str!("print.css"));

/// `@font-face` CSS embedding STIX Two Math (OFL, see assets/STIX-LICENSE.txt)
/// as a data URI. Added only to pages containing math (~540KB), so decks
/// render at TeX quality even on machines without a math font installed.
pub fn math_font_css() -> &'static str {
    use base64::Engine as _;
    use std::sync::OnceLock;
    static CSS: OnceLock<String> = OnceLock::new();
    CSS.get_or_init(|| {
        let woff2 = include_bytes!("../../assets/stix-two-math.woff2");
        let b64 = base64::engine::general_purpose::STANDARD.encode(woff2);
        format!(
            "@font-face {{ font-family: 'STIX Two Math'; \
             src: url(data:font/woff2;base64,{b64}) format('woff2'); font-display: swap; }}\n\
             math {{ font-family: 'STIX Two Math', math; }}"
        )
    })
}

/// Hot-reload client injected in `serve` mode.
/// Long-polls for changed `<section>` HTML and patches the DOM.
pub const LIVE_JS: &str = r#"
(async () => {
  let v = window.__MIRZAM_V__;
  while (true) {
    try {
      const res = await fetch('/events?v=' + v);
      const j = await res.json();
      if (j.v === v) continue;
      v = j.v;
      if (j.full) { location.reload(); return; }
      for (const [i, html] of j.changes) {
        const sec = document.querySelector(`section.slide[data-index="${i}"]`);
        if (sec) sec.outerHTML = html;
      }
      if (window.__mirzamRefresh) window.__mirzamRefresh();
    } catch (e) {
      await new Promise(r => setTimeout(r, 1000));
    }
  }
})();
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_css_is_tokens_then_base() {
        assert!(DEFAULT_CSS.trim_start().starts_with(":root {"));
        assert!(DEFAULT_CSS.contains("--mz-accent1"));
        assert!(DEFAULT_CSS.contains("* { box-sizing: border-box; }"));
    }

    #[test]
    fn default_css_carries_the_debug_overlay_rules() {
        assert!(DEFAULT_CSS.contains("html.mz-debug"));
        assert!(DEFAULT_CSS.contains("attr(data-pane)"));
    }

    #[test]
    fn viewer_js_handles_the_debug_toggle() {
        assert!(VIEWER_JS.contains("'l'"));
        assert!(VIEWER_JS.contains("mz-debug"));
    }

    #[test]
    fn print_css_neutralizes_the_debug_overlay() {
        assert!(PRINT_CSS.contains("html.mz-debug"));
    }
}
