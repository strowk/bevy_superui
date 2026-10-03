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
pub struct JsSource(pub String);

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
        Ok(JsSource(read_to_string(reader).await?))
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

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        lc: &mut LoadContext<'_>,
    ) -> Result<JsSource, std::io::Error> {
        let src = read_to_string(reader).await?;
        let tsx = lc.path().path().extension().and_then(|e| e.to_str()) != Some("ts");
        let module_id = Some(lc.path().path().to_string_lossy().into_owned());
        let opts = supersolid::TranspileOptions { tsx, module_id, ..Default::default() };
        let mut result = supersolid::transpile(&src, &opts);
        for d in &result.diagnostics {
            bevy::log::warn!("supersolid: {}", d.message);
        }

        // Resolve `.json` imports the transpiler recorded (file I/O is the
        // loader's job, not the pure transpiler's). Each resolved binding is
        // read via `read_asset_bytes`, which also registers the JSON file as
        // a load dependency so editing it triggers hot reload of this module.
        let importer = lc.path().path().to_string_lossy().into_owned();
        let dir = superui_paths::parent_dir(&importer);
        let mut json_prelude = String::new();
        for (binding, specifier) in &result.json_imports {
            let asset_path = superui_paths::join_asset(dir, specifier);
            let text = match lc.read_asset_bytes(asset_path.clone()).await {
                Ok(bytes) => match String::from_utf8(bytes) {
                    Ok(text) => text,
                    Err(e) => {
                        bevy::log::warn!(
                            "supersolid: JSON import \"{specifier}\" ({asset_path}) is not valid UTF-8: {e}"
                        );
                        continue;
                    }
                },
                Err(e) => {
                    bevy::log::warn!(
                        "supersolid: JSON import \"{specifier}\" ({asset_path}) could not be read: {e}"
                    );
                    continue;
                }
            };
            match supersolid::json_binding(binding, &text) {
                Ok(line) => json_prelude.push_str(&line),
                Err(e) => {
                    bevy::log::warn!(
                        "supersolid: JSON import \"{specifier}\" ({asset_path}) is invalid: {e}"
                    );
                }
            }
        }
        if !json_prelude.is_empty() {
            result.code = format!("{json_prelude}{}", result.code);
        }

        // Graceful degradation (design §1): return whatever JS was produced even on
        // diagnostics; never fail the load for a transpile warning.
        Ok(JsSource(result.code))
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
        assert_eq!(jss.get(&js).unwrap().0, "var a = 1;");
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
        let out = &jss.get(&handle).unwrap().0;
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
        let out = &jss.get(&handle).unwrap().0;
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
        let out = &jss.get(&handle).unwrap().0;
        let re_spaced = out.contains(r#"const data = {"n": 7};"#);
        assert!(
            out.contains(r#"const data = {"n":7};"#) || re_spaced,
            "loader must inline the JSON import as a top-level const:\n{out}"
        );
    }
}
