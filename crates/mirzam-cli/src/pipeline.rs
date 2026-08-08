//! build / serve 共通のビルドパイプライン。
//! スライド単位のソースハッシュでレンダリング結果をキャッシュし、
//! 変更されたスライドだけを再レンダリングする。

use std::collections::{BTreeSet, HashMap};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

pub struct BuildOutput {
    pub meta: mirzam_core::DeckMeta,
    /// レンダリング済み `<section>` HTML(スライド順)
    pub sections: Vec<String>,
    /// スライドごとのソースハッシュ(差分検出用)
    pub hashes: Vec<u64>,
    pub warnings: Vec<String>,
    /// このデッキを構成するソースファイル(監視対象)
    pub files: BTreeSet<PathBuf>,
    /// 今回のビルドで実際に再レンダリングされた枚数(キャッシュミス数)
    pub rendered: usize,
}

pub type RenderCache = HashMap<u64, String>;

pub fn build_deck(input: &Path, cache: &mut RenderCache) -> Result<BuildOutput, String> {
    let src = std::fs::read_to_string(input)
        .map_err(|e| format!("{} を読めません: {e}", input.display()))?;
    let base_dir = input
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .to_path_buf();

    let mut files: BTreeSet<PathBuf> = BTreeSet::new();
    files.insert(input.to_path_buf());

    // 1. frontmatter
    let (fm, body) = mirzam_syntax::split_frontmatter(&src);
    let meta = match fm {
        Some(yaml) => mirzam_core::parse_meta(yaml)?,
        None => mirzam_core::DeckMeta::default(),
    };

    // 2. include 展開(読んだファイルを収集)
    let body = mirzam_syntax::expand_includes_tracked(
        body,
        &base_dir,
        &mirzam_syntax::FsProvider,
        &mut files,
    );

    // 3. 変数置換(コードフェンス内は対象外)。
    //    変数の変更は置換後ソースの変化としてスライドハッシュに反映される。
    let vars = meta.var_table();
    let body = substitute_outside_fences(&body, &vars);

    // 4. スライド分割 → スライド単位でキャッシュ参照レンダリング
    let slide_sources = mirzam_syntax::split_slides(&body);
    let mut sections = Vec::with_capacity(slide_sources.len());
    let mut hashes = Vec::with_capacity(slide_sources.len());
    let mut warnings = Vec::new();
    let mut rendered = 0usize;

    for (i, slide_src) in slide_sources.iter().enumerate() {
        // ハッシュにはスライド位置も含める(data-index がセクション HTML に埋まるため)
        let key = slide_hash(slide_src, i);
        hashes.push(key);
        match cache.get(&key) {
            Some(html) => sections.push(html.clone()),
            None => {
                let slide = mirzam_syntax::parse_slide(slide_src);
                let out = mirzam_render::render_slide_html(&slide, i, &base_dir);
                warnings.extend(out.warnings);
                cache.insert(key, out.html.clone());
                sections.push(out.html);
                rendered += 1;
            }
        }
    }

    // キャッシュの野放図な成長を防ぐ(編集セッションでは十分な上限)
    if cache.len() > 4096 {
        cache.retain(|k, _| hashes.contains(k));
    }

    Ok(BuildOutput {
        meta,
        sections,
        hashes,
        warnings,
        files,
        rendered,
    })
}

fn slide_hash(src: &str, index: usize) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    src.hash(&mut h);
    index.hash(&mut h);
    h.finish()
}

/// コードフェンス外の行にのみ変数置換を適用する
fn substitute_outside_fences(
    body: &str,
    vars: &std::collections::BTreeMap<String, mirzam_core::Value>,
) -> String {
    let mut out = String::with_capacity(body.len());
    let mut in_code = false;
    for line in body.lines() {
        if line.trim_start().starts_with("```") {
            in_code = !in_code;
            out.push_str(line);
        } else if in_code {
            out.push_str(line);
        } else {
            out.push_str(&mirzam_core::substitute_vars(line, vars));
        }
        out.push('\n');
    }
    out
}
