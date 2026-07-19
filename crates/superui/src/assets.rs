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
}
