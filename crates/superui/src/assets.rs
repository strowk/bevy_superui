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

#[derive(Default)]
pub struct HtmlLoader;
#[derive(Default)]
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
/// (so mount/hot-reload treat it identically to hand-written `.js`). Native-only:
/// `oxc` must not enter the wasm binary (direction spec §11.3).
#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
pub struct TsxLoader;

#[cfg(not(target_arch = "wasm32"))]
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
        let tsx = lc.path().extension().and_then(|e| e.to_str()) != Some("ts");
        let module_id = Some(lc.path().to_string_lossy().into_owned());
        let opts = supersolid::TranspileOptions { tsx, module_id, ..Default::default() };
        let result = supersolid::transpile(&src, &opts);
        for d in &result.diagnostics {
            bevy::log::warn!("supersolid: {}", d.message);
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
    use bevy::asset::io::{AssetSource, AssetSourceId};
    use bevy::asset::{AssetApp, AssetPlugin, AssetServer, LoadState};

    #[test]
    fn loads_html_and_js_sources() {
        let dir = Dir::new("assets".into());
        dir.insert_asset("ui.html".as_ref(), b"<div id='x'></div>");
        dir.insert_asset("app.js".as_ref(), b"var a = 1;");

        let mut app = App::new();
        app.register_asset_source(
            AssetSourceId::Default,
            AssetSource::build()
                .with_reader(move || Box::new(MemoryAssetReader { root: dir.clone() })),
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
            AssetSource::build().with_reader(move || Box::new(MemoryAssetReader { root: dir.clone() })),
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
            AssetSource::build().with_reader(move || Box::new(MemoryAssetReader { root: dir.clone() })),
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
}
