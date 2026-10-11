//! Asset types + loaders for authored `.html` and `.js`. The `.css` loader comes
//! from flair via `SuperUiCssPlugin`. Loaders keep raw source; the HTML is parsed
//! and the JS executed at mount time (so hot reload can re-parse / re-exec).

use bevy::asset::io::Reader;
use bevy::asset::{Asset, AssetLoader, LoadContext};
use bevy::prelude::*;
use bevy::reflect::TypePath;

/// Raw authored HTML source (parsed into a `Dom` at mount).
#[derive(Asset, TypePath, Debug, Clone)]
pub struct HtmlSource(pub String);

/// Raw authored JS source (executed against the DOM at mount).
#[derive(Asset, TypePath, Debug, Clone)]
pub struct JsSource {
    pub code: String,
    pub css: String,
}

#[derive(Default, TypePath)]
pub struct HtmlLoader;
#[derive(Default, TypePath)]
pub struct JsLoader;

async fn read_to_string(reader: &mut dyn Reader) -> Result<String, std::io::Error> {
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).await?;
    String::from_utf8(bytes).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

impl AssetLoader for HtmlLoader {
    type Asset = HtmlSource;
    type Settings = ();
    type Error = std::io::Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _lc: &mut LoadContext<'_>,
    ) -> Result<HtmlSource, std::io::Error> {
        Ok(HtmlSource(read_to_string(reader).await?))
    }

    fn extensions(&self) -> &[&str] {
        &["html"]
    }
}

impl AssetLoader for JsLoader {
    type Asset = JsSource;
    type Settings = ();
    type Error = std::io::Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _lc: &mut LoadContext<'_>,
    ) -> Result<JsSource, std::io::Error> {
        Ok(JsSource {
            code: read_to_string(reader).await?,
            css: String::new(),
        })
    }

    fn extensions(&self) -> &[&str] {
        &["js"]
    }
}

/// Loads `.tsx`/`.ts`, transpiles via `supersolid`, and yields a `JsSource`
/// (so mount/hot-reload treat it identically to hand-written `.js`). Compiled
/// and registered whenever `any(not(wasm32), feature = "transpiler")`: always
/// on native, and on wasm only under the `transpiler` feature (the web
/// playground's in-browser oxc path).
#[cfg(any(not(target_arch = "wasm32"), feature = "transpiler"))]
#[derive(Default, TypePath)]
pub struct TsxLoader;

#[cfg(any(not(target_arch = "wasm32"), feature = "transpiler"))]
impl AssetLoader for TsxLoader {
    type Asset = JsSource;
    type Settings = ();
    type Error = std::io::Error;

    /// Async BFS over the import graph rooted at this asset (design §4 / spec
    /// §4): per module, `transpile_module`, then resolve each `dep`/`.css`/
    /// `.json` import via `resolve_candidates` + `read_asset_bytes`. Every id
    /// read this way (modules, css, json) becomes a load dependency, so
    /// editing *any* graph file — not just the entry — re-runs this loader
    /// (graph-wide HMR). Graceful degradation throughout: a missing/invalid
    /// import warns and degrades (sentinel edge / skipped css or json) rather
    /// than failing the load; only a hard I/O error reading the entry itself
    /// (via `reader`) propagates as `Err`.
    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        lc: &mut LoadContext<'_>,
    ) -> Result<JsSource, std::io::Error> {
        use std::collections::{BTreeMap, HashSet, VecDeque};
        use supersolid::{
            assemble, json_binding, resolve_candidates, transpile_module, ResolvedModule,
            DEFAULT_MODULES_ROOT,
        };

        let entry_id = lc.path().path().to_string_lossy().into_owned();

        let mut modules: BTreeMap<String, ResolvedModule> = BTreeMap::new();
        let mut seen: HashSet<String> = HashSet::new();
        let mut queue: VecDeque<String> = VecDeque::new();
        seen.insert(entry_id.clone());
        queue.push_back(entry_id.clone());

        let mut entry_bytes = Some({
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;
            bytes
        });

        while let Some(id) = queue.pop_front() {
            // The entry's bytes are already in `reader`; every other id is a
            // dependency read via `read_asset_bytes` (registers it for HMR).
            let bytes = if id == entry_id && entry_bytes.is_some() {
                entry_bytes.take().unwrap()
            } else {
                match lc.read_asset_bytes(id.clone()).await {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        bevy::log::warn!("supersolid: could not read module {id:?}: {e}");
                        continue;
                    }
                }
            };
            let source = String::from_utf8_lossy(&bytes).into_owned();

            let tsx = !id.ends_with(".ts");
            let opts = supersolid::TranspileOptions {
                tsx,
                module_id: Some(id.clone()),
                ..Default::default()
            };
            let m = transpile_module(&source, &opts);
            for d in &m.diagnostics {
                bevy::log::warn!("supersolid: {}", d.message);
            }

            let dir = superui_paths::parent_dir(&id);

            let mut edges = Vec::with_capacity(m.deps.len());
            for spec in &m.deps {
                let candidates = resolve_candidates(&id, spec, DEFAULT_MODULES_ROOT);
                let mut resolved_id = None;
                for candidate in &candidates {
                    if lc.read_asset_bytes(candidate.clone()).await.is_ok() {
                        resolved_id = Some(candidate.clone());
                        break;
                    }
                }
                match resolved_id {
                    Some(target) => {
                        if seen.insert(target.clone()) {
                            queue.push_back(target.clone());
                        }
                        edges.push((spec.clone(), target));
                    }
                    None => {
                        bevy::log::warn!(
                            "supersolid: could not resolve {spec:?} imported from {id:?}"
                        );
                        edges.push((spec.clone(), format!("superui:unresolved:{spec}")));
                    }
                }
            }

            let mut css = String::new();
            for spec in &m.css_imports {
                let path = superui_paths::join_asset(dir, spec);
                match lc.read_asset_bytes(path.clone()).await {
                    Ok(bytes) => css.push_str(&String::from_utf8_lossy(&bytes)),
                    Err(e) => {
                        bevy::log::warn!("supersolid: could not read {path:?} ({spec:?}): {e}");
                    }
                }
            }

            let mut json_prelude = String::new();
            for (binding, spec) in &m.json_imports {
                let path = superui_paths::join_asset(dir, spec);
                match lc.read_asset_bytes(path.clone()).await {
                    Ok(bytes) => match String::from_utf8(bytes) {
                        Ok(text) => match json_binding(binding, &text) {
                            Ok(line) => json_prelude.push_str(&line),
                            Err(e) => {
                                bevy::log::warn!("supersolid: {path:?} ({spec:?}): {e}");
                            }
                        },
                        Err(e) => {
                            bevy::log::warn!(
                                "supersolid: JSON import \"{spec}\" ({path}) is not valid UTF-8: {e}"
                            );
                        }
                    },
                    Err(e) => {
                        bevy::log::warn!("supersolid: could not read {path:?} ({spec:?}): {e}");
                    }
                }
            }

            modules.insert(
                id,
                ResolvedModule { factory_code: m.factory_code, edges, css, json_prelude },
            );
        }

        let bundle = assemble(&entry_id, &modules);
        for d in &bundle.diagnostics {
            bevy::log::warn!("supersolid: {}", d.message);
        }

        // Graceful degradation (design §1): return whatever JS was produced even on
        // diagnostics; never fail the load for a resolve/transpile/css/json warning.
        Ok(JsSource { code: bundle.code, css: bundle.css })
    }

    fn extensions(&self) -> &[&str] {
        &["tsx", "ts"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::io::memory::{Dir, MemoryAssetReader};
    use bevy::asset::io::{AssetSourceBuilder, AssetSourceId};
    use bevy::asset::{AssetApp, AssetPlugin, AssetServer, LoadState};

    #[test]
    fn loads_html_and_js_sources() {
        let dir = Dir::new("assets".into());
        dir.insert_asset("ui.html".as_ref(), b"<div id='x'></div>");
        dir.insert_asset("app.js".as_ref(), b"var a = 1;");

        let mut app = App::new();
        app.register_asset_source(
            AssetSourceId::Default,
            AssetSourceBuilder::new(move || Box::new(MemoryAssetReader { root: dir.clone() })),
        );
        app.add_plugins((
            bevy::app::TaskPoolPlugin::default(),
            AssetPlugin::default(),
        ));
        app.init_asset::<HtmlSource>()
            .init_asset::<JsSource>()
            .register_asset_loader(HtmlLoader)
            .register_asset_loader(JsLoader);
        app.finish();

        let (html, js) = {
            let server = app.world().resource::<AssetServer>().clone();
            (
                server.load::<HtmlSource>("ui.html"),
                server.load::<JsSource>("app.js"),
            )
        };
        for _ in 0..64 {
            app.update();
            let server = app.world().resource::<AssetServer>();
            if matches!(server.load_state(html.id()), LoadState::Loaded)
                && matches!(server.load_state(js.id()), LoadState::Loaded)
            {
                break;
            }
        }
        let htmls = app.world().resource::<Assets<HtmlSource>>();
        let jss = app.world().resource::<Assets<JsSource>>();
        assert_eq!(htmls.get(&html).unwrap().0, "<div id='x'></div>");
        assert_eq!(jss.get(&js).unwrap().code, "var a = 1;");
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn tsx_loader_bakes_module_path_into_hot_id() {
        let dir = Dir::new("assets".into());
        dir.insert_asset(
            "counter.tsx".as_ref(),
            b"function Counter(){ return <div/>; } render(() => <Counter/>, root);",
        );

        let mut app = App::new();
        app.register_asset_source(
            AssetSourceId::Default,
            AssetSourceBuilder::new(move || Box::new(MemoryAssetReader { root: dir.clone() })),
        );
        app.add_plugins((bevy::app::TaskPoolPlugin::default(), AssetPlugin::default()));
        app.init_asset::<JsSource>().register_asset_loader(TsxLoader);
        app.finish();

        let handle = {
            let server = app.world().resource::<AssetServer>().clone();
            server.load::<JsSource>("counter.tsx")
        };
        for _ in 0..64 {
            app.update();
            if matches!(
                app.world().resource::<AssetServer>().load_state(handle.id()),
                LoadState::Loaded
            ) {
                break;
            }
        }
        let jss = app.world().resource::<Assets<JsSource>>();
        let out = &jss.get(&handle).unwrap().code;
        assert!(
            out.contains(r#"$ss.hot("counter.tsx#Counter", Counter)"#),
            "loader must bake the asset path into the HMR id:\n{out}"
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn tsx_loader_transpiles_to_jssource() {
        let dir = Dir::new("assets".into());
        dir.insert_asset(
            "app.tsx".as_ref(),
            b"const n: number = 1; const a = <div class=\"x\">{n}</div>;",
        );

        let mut app = App::new();
        app.register_asset_source(
            AssetSourceId::Default,
            AssetSourceBuilder::new(move || Box::new(MemoryAssetReader { root: dir.clone() })),
        );
        app.add_plugins((bevy::app::TaskPoolPlugin::default(), AssetPlugin::default()));
        app.init_asset::<JsSource>().register_asset_loader(TsxLoader);
        app.finish();

        let handle = {
            let server = app.world().resource::<AssetServer>().clone();
            server.load::<JsSource>("app.tsx")
        };
        for _ in 0..64 {
            app.update();
            if matches!(
                app.world().resource::<AssetServer>().load_state(handle.id()),
                LoadState::Loaded
            ) {
                break;
            }
        }
        let jss = app.world().resource::<Assets<JsSource>>();
        let out = &jss.get(&handle).unwrap().code;
        assert!(!out.contains(": number"), "types stripped by loader:\n{out}");
        assert!(out.contains(r#"$ss.el("div")"#), "JSX lowered by loader:\n{out}");
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn tsx_loader_inlines_json_imports() {
        let dir = Dir::new("assets".into());
        dir.insert_asset("data.json".as_ref(), br#"{"n":7}"#);
        dir.insert_asset(
            "app.tsx".as_ref(),
            br#"import data from "./data.json"; const a = <div>{data.n}</div>;"#,
        );

        let mut app = App::new();
        app.register_asset_source(
            AssetSourceId::Default,
            AssetSourceBuilder::new(move || Box::new(MemoryAssetReader { root: dir.clone() })),
        );
        app.add_plugins((bevy::app::TaskPoolPlugin::default(), AssetPlugin::default()));
        app.init_asset::<JsSource>().register_asset_loader(TsxLoader);
        app.finish();

        let handle = {
            let server = app.world().resource::<AssetServer>().clone();
            server.load::<JsSource>("app.tsx")
        };
        for _ in 0..64 {
            app.update();
            if matches!(
                app.world().resource::<AssetServer>().load_state(handle.id()),
                LoadState::Loaded
            ) {
                break;
            }
        }
        let jss = app.world().resource::<Assets<JsSource>>();
        let out = &jss.get(&handle).unwrap().code;
        let re_spaced = out.contains(r#"const data = {"n": 7};"#);
        assert!(
            out.contains(r#"const data = {"n":7};"#) || re_spaced,
            "loader must inline the JSON import as a top-level const:\n{out}"
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn tsx_loader_missing_json_import_loads_without_inlining() {
        // Loader graceful degradation (design §1): a missing `.json` import must
        // never fail the `.tsx` load — it warns and skips inlining that binding.
        let dir = Dir::new("assets".into());
        dir.insert_asset(
            "app.tsx".as_ref(),
            br#"import data from "./missing.json"; const a = <div>{data}</div>;"#,
        );

        let mut app = App::new();
        app.register_asset_source(
            AssetSourceId::Default,
            AssetSourceBuilder::new(move || Box::new(MemoryAssetReader { root: dir.clone() })),
        );
        app.add_plugins((bevy::app::TaskPoolPlugin::default(), AssetPlugin::default()));
        app.init_asset::<JsSource>().register_asset_loader(TsxLoader);
        app.finish();

        let handle = {
            let server = app.world().resource::<AssetServer>().clone();
            server.load::<JsSource>("app.tsx")
        };
        for _ in 0..64 {
            app.update();
            if matches!(
                app.world().resource::<AssetServer>().load_state(handle.id()),
                LoadState::Loaded
            ) {
                break;
            }
        }
        let load_state = app.world().resource::<AssetServer>().load_state(handle.id());
        assert!(
            matches!(load_state, LoadState::Loaded),
            "missing JSON import must not fail the load, got {load_state:?}"
        );
        let jss = app.world().resource::<Assets<JsSource>>();
        let out = &jss.get(&handle).unwrap().code;
        assert!(
            !out.contains("const data ="),
            "no inlined const for a missing JSON import:\n{out}"
        );
    }

    /// Shared setup for the graph-walk tests below: a `Dir` + app wired with
    /// `TsxLoader`, loading `path` to a `Loaded` `JsSource`.
    #[cfg(not(target_arch = "wasm32"))]
    fn load_tsx(dir: Dir, path: &str) -> (App, Handle<JsSource>) {
        let mut app = App::new();
        app.register_asset_source(
            AssetSourceId::Default,
            AssetSourceBuilder::new(move || Box::new(MemoryAssetReader { root: dir.clone() })),
        );
        app.add_plugins((bevy::app::TaskPoolPlugin::default(), AssetPlugin::default()));
        app.init_asset::<JsSource>().register_asset_loader(TsxLoader);
        app.finish();

        let handle = {
            let server = app.world().resource::<AssetServer>().clone();
            server.load::<JsSource>(path.to_string())
        };
        for _ in 0..64 {
            app.update();
            if matches!(
                app.world().resource::<AssetServer>().load_state(handle.id()),
                LoadState::Loaded
            ) {
                break;
            }
        }
        (app, handle)
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn tsx_loader_bundles_a_two_file_graph() {
        // `app.tsx` imports a sibling module; the bundle must define both
        // modules (singleton registry, spec §1) and run the entry — proving
        // the loader walks the import graph rather than transpiling only the
        // one file it was asked to load.
        let dir = Dir::new("assets".into());
        dir.insert_asset(
            "counter.tsx".as_ref(),
            br#"export function Counter(){ return <div class="c"/>; }"#,
        );
        dir.insert_asset(
            "app.tsx".as_ref(),
            br#"import { Counter } from "./counter.tsx"; render(() => <Counter/>, root);"#,
        );

        let (app, handle) = load_tsx(dir, "app.tsx");
        assert!(
            matches!(
                app.world().resource::<AssetServer>().load_state(handle.id()),
                LoadState::Loaded
            ),
            "a two-file graph must still load"
        );
        let jss = app.world().resource::<Assets<JsSource>>();
        let out = &jss.get(&handle).unwrap().code;
        assert!(out.contains(r#"$ssmod.def("app.tsx""#), "entry defined:\n{out}");
        assert!(out.contains(r#"$ssmod.def("counter.tsx""#), "dep defined:\n{out}");
        assert!(out.contains(r#"$ss.el("div")"#), "counter's JSX lowered into the bundle:\n{out}");
        assert!(out.contains(r#"$ssmod.run("app.tsx")"#), "entry run:\n{out}");
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn tsx_loader_missing_dep_degrades_gracefully() {
        // Graceful degradation (design §1): an unresolvable import must not
        // fail the load — it warns and the bundle gets a sentinel edge.
        let dir = Dir::new("assets".into());
        dir.insert_asset(
            "app.tsx".as_ref(),
            br#"import { Nope } from "./gone.tsx"; const a = <div>{Nope}</div>;"#,
        );

        let (app, handle) = load_tsx(dir, "app.tsx");
        let load_state = app.world().resource::<AssetServer>().load_state(handle.id());
        assert!(
            matches!(load_state, LoadState::Loaded),
            "a missing dep must not fail the load, got {load_state:?}"
        );
        let jss = app.world().resource::<Assets<JsSource>>();
        let out = &jss.get(&handle).unwrap().code;
        assert!(
            out.contains("superui:unresolved:"),
            "unresolved dep must degrade to a sentinel edge:\n{out}"
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn tsx_loader_collects_module_css_onto_jssource() {
        // A module's own `.css` import must flow onto the loaded `JsSource.css`
        // (graph-wide CSS concatenation, spec §5), not just the entry's.
        let dir = Dir::new("assets".into());
        dir.insert_asset("c.css".as_ref(), b".c{color:red}");
        dir.insert_asset(
            "app.tsx".as_ref(),
            br#"import "./c.css"; const a = <div class="c"/>;"#,
        );

        let (app, handle) = load_tsx(dir, "app.tsx");
        let jss = app.world().resource::<Assets<JsSource>>();
        let css = &jss.get(&handle).unwrap().css;
        assert!(css.contains(".c{color:red}"), "module css collected onto JsSource.css:\n{css}");
    }
}
