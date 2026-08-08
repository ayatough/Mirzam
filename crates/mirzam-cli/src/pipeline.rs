//! build / serve 共通のビルドパイプライン。
//! スライド単位のソースハッシュでレンダリング結果をキャッシュし、
//! 変更されたスライドだけを再レンダリングする。
//! キャッシュはスライドが参照する画像等の mtime も検証するため、
//! 画像ファイルだけを差し替えた場合も該当スライドだけが再レンダリングされる。

use std::collections::{BTreeSet, HashMap};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub struct BuildOutput {
    pub meta: mirzam_core::DeckMeta,
    /// レンダリング済み `<section>` HTML(スライド順)
    pub sections: Vec<String>,
    /// スライドごとの出力 HTML のハッシュ(差分検出用)。
    /// ソースではなく出力のハッシュなので、画像ファイルの内容だけが
    /// 変わった場合(ソース不変)も配信すべき差分として検出できる
    pub hashes: Vec<u64>,
    /// ページレベル設定(タイトル・アスペクト・カスタム CSS)の指紋。
    /// スライドは同一でもページの再組み立てが必要な変更を検出する
    pub page_fingerprint: u64,
    /// frontmatter `css:` の内容(解決済み)
    pub custom_css: Option<String>,
    pub warnings: Vec<String>,
    /// このデッキを構成するソースファイル + 参照アセット(監視対象)
    pub files: BTreeSet<PathBuf>,
    /// このビルドで実際に再レンダリングされた枚数(キャッシュミス数)
    pub rendered: usize,
}

pub struct CacheEntry {
    html: String,
    /// レンダリング時に参照したアセットとその mtime
    assets: Vec<(PathBuf, Option<SystemTime>)>,
}

impl CacheEntry {
    /// 参照アセットが当時と同じ mtime のままか
    fn is_fresh(&self) -> bool {
        self.assets.iter().all(|(p, t)| mtime(p) == *t)
    }
}

pub type RenderCache = HashMap<u64, CacheEntry>;

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
    let mut warnings = Vec::new();

    // カスタム CSS の読み込み(失敗は警告に留める)
    let custom_css = match &meta.css {
        Some(rel) => {
            let path = base_dir.join(rel);
            files.insert(path.clone());
            match std::fs::read_to_string(&path) {
                Ok(css) => Some(css),
                Err(e) => {
                    warnings.push(format!("css: {rel} を読めません: {e}"));
                    None
                }
            }
        }
        None => None,
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
    let mut rendered = 0usize;

    for (i, slide_src) in slide_sources.iter().enumerate() {
        // キャッシュキーにはスライド位置も含める(data-index がセクション HTML に埋まるため)
        let key = slide_hash(slide_src, i);
        match cache.get(&key).filter(|e| e.is_fresh()) {
            Some(entry) => {
                for (p, _) in &entry.assets {
                    files.insert(p.clone());
                }
                hashes.push(str_hash(&entry.html));
                sections.push(entry.html.clone());
            }
            None => {
                let slide = mirzam_syntax::parse_slide(slide_src);
                let out = mirzam_render::render_slide_html(&slide, i, &base_dir);
                warnings.extend(out.warnings);
                let assets: Vec<(PathBuf, Option<SystemTime>)> = out
                    .assets
                    .iter()
                    .map(|p| (p.clone(), mtime(p)))
                    .collect();
                for (p, _) in &assets {
                    files.insert(p.clone());
                }
                cache.insert(
                    key,
                    CacheEntry {
                        html: out.html.clone(),
                        assets,
                    },
                );
                hashes.push(str_hash(&out.html));
                sections.push(out.html);
                rendered += 1;
            }
        }
    }

    // キャッシュの野放図な成長を防ぐ(編集セッションでは十分な上限)
    if cache.len() > 4096 {
        let live_keys: std::collections::HashSet<u64> = slide_sources
            .iter()
            .enumerate()
            .map(|(i, s)| slide_hash(s, i))
            .collect();
        cache.retain(|k, _| live_keys.contains(k));
    }

    let page_fingerprint = {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        meta.title.hash(&mut h);
        meta.author.hash(&mut h);
        meta.aspect.hash(&mut h);
        custom_css.hash(&mut h);
        // 数式の有無で数式フォントの同梱が変わる(serve は全体リロードで反映)
        mirzam_render::sections_have_math(&sections).hash(&mut h);
        h.finish()
    };

    Ok(BuildOutput {
        meta,
        sections,
        hashes,
        page_fingerprint,
        custom_css,
        warnings,
        files,
        rendered,
    })
}

fn mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn slide_hash(src: &str, index: usize) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    src.hash(&mut h);
    index.hash(&mut h);
    h.finish()
}

fn str_hash(s: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
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
