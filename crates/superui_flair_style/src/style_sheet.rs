use crate::{StyleBlock, StyleFontFace, VarTokens, builder::StyleSheetBuilder};

use std::borrow::Borrow;

use crate::animations::*;
use crate::components::StyleData;

use crate::css_selector::CssSelector;
use crate::layers::LayersHierarchy;

use crate::media_selector::MediaFeaturesProvider;
use bevy_asset::{Asset, AssetId, Handle};
use bevy_reflect::TypePath;
use bevy_text::FontSource;
use rustc_hash::{FxBuildHasher, FxHashMap};
use std::sync::Arc;

pub(crate) trait StyleMatchableElement:
    selectors::Element<Impl = crate::css_selector::CssSelectorImpl> + Borrow<StyleData>
{
}

impl<T> StyleMatchableElement for T where
    T: selectors::Element<Impl = crate::css_selector::CssSelectorImpl> + Borrow<StyleData>
{
}

/// Trait for resolving variable names to their associated [`VarTokens`].
///
/// A `VarResolver` is typically used during the dynamic evaluation of styles, where variable
/// references (e.g., `var(--my-color)`) need to be resolved into their underlying token
/// representations.
///
/// Implementors of this trait provide access to all known variable names, and can fetch the
/// corresponding [`VarTokens`] for a specific variable name.
///
/// This trait is used internally by the style engine to support dynamic and scoped variables.
pub trait VarResolver {
    /// Returns a set of all variable names that can be resolved.
    fn get_all_names(&self) -> Vec<Arc<str>>;

    /// Attempts to retrieve the [`VarTokens`] associated with a given variable name.
    fn get_var_tokens(&self, var_name: &str) -> Option<&'_ VarTokens>;
}

impl VarResolver for &dyn VarResolver {
    fn get_all_names(&self) -> Vec<Arc<str>> {
        (*self).get_all_names()
    }

    fn get_var_tokens(&self, var_name: &str) -> Option<&'_ VarTokens> {
        (*self).get_var_tokens(var_name)
    }
}

/// Represents a collection of styles that can be applied to elements,
/// storing rules for various style properties.
#[derive(Debug, Clone, TypePath, Asset)]
pub struct StyleSheet {
    // This only make sense here when importing other stylesheets
    pub(super) font_faces: Vec<StyleFontFace>,
    pub(super) resolved_font_faces: FxHashMap<String, FontSource>,
    // Only used to keep a strong Handle to the used blocks
    #[dependency]
    pub(super) block_handles: Vec<Handle<StyleBlock>>,
    pub(super) animation_keyframes: FxHashMap<Arc<str>, AnimationKeyframes>,
    pub(super) css_selectors_to_blocks: Vec<(CssSelector, AssetId<StyleBlock>)>,
}

impl StyleSheet {
    /// Creates a builder to create a new [`StyleSheet`].
    pub fn builder() -> StyleSheetBuilder {
        StyleSheetBuilder::new()
    }

    /// Creates an empty [`StyleSheet`].
    pub const fn empty() -> Self {
        Self {
            font_faces: Vec::new(),
            resolved_font_faces: FxHashMap::with_hasher(FxBuildHasher),
            block_handles: Vec::new(),
            animation_keyframes: FxHashMap::with_hasher(FxBuildHasher),
            css_selectors_to_blocks: Vec::new(),
        }
    }

    // >>> SUPERUI-FORK-PATCH: stylesheet-extend  (docs/fork-patches.md#stylesheet-extend)
    /// Append `other`'s rules after self's, then re-sort by (layer, specificity)
    /// so the combined sheet cascades correctly (other wins only on a true tie).
    ///
    /// `font_faces`, `block_handles`, and `css_selectors_to_blocks` are appended
    /// in order; `resolved_font_faces` and `animation_keyframes` are merged by
    /// key, with `other`'s entries overwriting `self`'s on a name collision.
    ///
    /// `css_selectors_to_blocks` additionally gets re-sorted: each operand was
    /// individually sorted ascending by `(layer, specificity)` when it was
    /// built (see `StyleSheetBuilder::build`), but concatenating two sorted
    /// vectors does not produce a globally sorted one, and the resolver
    /// applies matches in array order with last-write-wins — it depends on
    /// that global order. The sort uses the exact same comparator the builder
    /// uses (`LayersHierarchy::cmp_layers` then `CssSelector::specificity`),
    /// rebuilding a `LayersHierarchy` from the already-concatenated list (self's
    /// entries first, so a layer name first seen in `self` keeps the priority
    /// position it would have had if `self` alone had defined it). The sort is
    /// STABLE, so entries with equal `(layer, specificity)` keep their
    /// concatenation order — `other`'s rules, appended after `self`'s, still
    /// win same-specificity ties, while a genuinely higher-specificity rule in
    /// either operand now correctly outranks a lower-specificity rule in the
    /// other, regardless of which sheet it came from.
    pub fn extend(&mut self, other: StyleSheet) {
        self.font_faces.extend(other.font_faces);
        self.resolved_font_faces.extend(other.resolved_font_faces);
        self.block_handles.extend(other.block_handles);
        self.animation_keyframes.extend(other.animation_keyframes);
        self.css_selectors_to_blocks.extend(other.css_selectors_to_blocks);

        let mut layers_hierarchy = LayersHierarchy::new();
        for (selector, _) in &self.css_selectors_to_blocks {
            layers_hierarchy.define_layer(&selector.layer);
        }
        self.css_selectors_to_blocks.sort_by(|(a, _), (b, _)| {
            layers_hierarchy
                .cmp_layers(&a.layer, &b.layer)
                .then_with(|| a.specificity().cmp(&b.specificity()))
        });
    }
    // <<< SUPERUI-FORK-PATCH: stylesheet-extend

    /// Returns block ids that matches with the given element.
    /// Results are sorted from less specific to more specific.
    pub(crate) fn get_matching_block_ids_for_element<
        E: StyleMatchableElement,
        M: MediaFeaturesProvider,
    >(
        &self,
        element: &E,
        media_provider: &M,
    ) -> Vec<AssetId<StyleBlock>> {
        self.css_selectors_to_blocks
            .iter()
            .filter_map(|(s, id)| {
                (s.matches_selector(element) && s.matches_media_selector(media_provider))
                    .then_some(*id)
            })
            .collect()
    }
}

#[cfg(all(test, not(miri)))]
mod tests {
    use crate::StyleResolver;
    use crate::animations::{
        AnimationConfiguration, AnimationKeyframes, AnimationOptions, AnimationProperty,
        AnimationPropertyId, EasingFunction, TransitionOptions, TransitionPropertyId,
    };
    use crate::asset_loader::{CustomLoader, StyleAssetLoader};
    use crate::builder::{BlockBuilder, StyleSheetBuilderBlockId};
    use crate::css_selector::CssSelector;
    use crate::media_selector::{MediaFeaturesProvider, MediaRangeSelector, MediaSelector};
    use crate::test_utils::NoVarsSupportedResolver;
    use crate::testing::{css_selector, entity};
    use crate::{ColorScheme, StyleBlock, StyleBuilderProperty, StyleSheetBuilder};
    use bevy_asset::{AssetId, AssetPath, Assets, Handle, UntypedHandle};
    use bevy_ecs::component::Component;
    use superui_flair_core::{
        ComponentProperties, CssPropertyRegistry, PropertyCanonicalName, PropertyRegistry,
        ReflectValue,
    };
    use bevy_reflect::{Reflect, TypeRegistry};
    use std::any::TypeId;
    use std::sync::{Arc, LazyLock};
    use std::time::Duration;

    #[derive(Component, ComponentProperties, Reflect, Default)]
    struct TestComponent {
        value: f32,
    }

    const TEST_PROPERTY: PropertyCanonicalName =
        PropertyCanonicalName::from_component_field::<TestComponent>("value");

    const CSS_TEST_PROPERTY: &str = "test-property";

    static EMPTY_TYPE_REGISTRY: LazyLock<TypeRegistry> = LazyLock::new(TypeRegistry::new);

    static PROPERTY_REGISTRY: LazyLock<PropertyRegistry> = LazyLock::new(|| {
        let mut registry = PropertyRegistry::new();
        registry.register::<TestComponent>();
        registry
    });

    static CSS_PROPERTY_REGISTRY: LazyLock<CssPropertyRegistry> = LazyLock::new(|| {
        let registry = CssPropertyRegistry::default();
        registry.register_property(CSS_TEST_PROPERTY, TEST_PROPERTY);
        registry
    });

    macro_rules! resolve {
        ($p:expr) => {{
            PROPERTY_REGISTRY
                .resolve($p)
                .unwrap_or_else(|e| panic!("{e}"))
        }};
    }

    macro_rules! properties {
        ($($k:expr => $v:expr),* $(,)?) => {
            superui_flair_core::PropertiesHashMap::from_iter([$((
                resolve!($k),
                $v
            ),)*])
        };
    }

    macro_rules! property_map {
        () => {
            PROPERTY_REGISTRY.create_unset_values_map()
        };
        ($($k:expr => $v:expr),* $(,)?) => {{
            let mut property_map = PROPERTY_REGISTRY.create_unset_values_map();
            property_map.extend([$((
                resolve!($k),
                $v
            ),)*]);
            property_map
        }};
    }

    macro_rules! get_properties {
        ($blocks:expr, $ids:expr) => {{
            let block_resolver = crate::StyleResolver::new(&$blocks, $ids);
            let mut property_map = PROPERTY_REGISTRY.create_unset_values_map();
            let mut origin = PROPERTY_REGISTRY.create_property_map(None);
            block_resolver.resolve_property_values(
                &PROPERTY_REGISTRY,
                &crate::test_utils::NoVarsSupportedResolver,
                &mut property_map,
                &mut origin,
            );
            property_map
        }};
    }

    macro_rules! element {
        ($($tt:tt)*) => {
            &crate::css_selector::testing::TestElementRef::new(&entity!($($tt)*))
        };
    }

    const TEST_ANIMATION_NAME: &str = "test-animation";

    #[derive(Default)]
    struct TestMediaProvider {
        scheme: Option<ColorScheme>,
        resolution: Option<f32>,
        width: Option<u32>,
        height: Option<u32>,
    }

    impl MediaFeaturesProvider for TestMediaProvider {
        fn get_color_scheme(&self) -> Option<ColorScheme> {
            self.scheme
        }

        fn get_resolution(&self) -> Option<f32> {
            self.resolution
        }

        fn get_viewport_width(&self) -> Option<u32> {
            self.width
        }

        fn get_viewport_height(&self) -> Option<u32> {
            self.height
        }
    }

    struct TestAssetLoader<'a>(&'a mut Assets<StyleBlock>);

    impl<'a> CustomLoader for TestAssetLoader<'a> {
        fn load(&self, type_id: TypeId, path: AssetPath<'_>) -> UntypedHandle {
            panic!("Cannot load {path} of type {type_id:?}");
        }

        fn add_style_block(
            &mut self,
            _label: Arc<str>,
            style_block: StyleBlock,
        ) -> Handle<StyleBlock> {
            self.0.add(style_block)
        }
    }

    fn translate_ids<const N: usize>(
        blocks: &Assets<StyleBlock>,
        ids: [StyleSheetBuilderBlockId; N],
    ) -> [AssetId<StyleBlock>; N] {
        ids.map(|id| {
            blocks
                .iter()
                .find(|(_, block)| block.original_id == Some(id))
                .expect("Id not found")
                .0
        })
    }

    #[test]
    fn test_style_sheet() {
        let mut builder = StyleSheetBuilder::new();

        let mut keyframes_builder = AnimationKeyframes::builder(TEST_ANIMATION_NAME);

        keyframes_builder
            .add_keyframe(0.0)
            .with_properties([(TEST_PROPERTY, 0.0)]);

        keyframes_builder
            .add_keyframe(1.0)
            .with_properties([(TEST_PROPERTY, 1.0)]);

        builder.add_animation_keyframes(keyframes_builder.build(&PROPERTY_REGISTRY).unwrap());

        let block_with_name_id = builder
            .new_ruleset()
            .with_css_selector(css_selector!("#test_name"))
            .with_transition_property(AnimationProperty::new_specific_property(
                TransitionPropertyId::PropertyName,
                [CSS_TEST_PROPERTY],
            ))
            .with_transition_property(AnimationProperty::new_specific_property(
                TransitionPropertyId::Duration,
                [Duration::from_secs(1)],
            ))
            .with_animation_property(AnimationProperty::new_shorthand_specific([[
                (AnimationPropertyId::Name, TEST_ANIMATION_NAME.into()),
                (AnimationPropertyId::Duration, Duration::from_secs(1).into()),
            ]]))
            .block_id();

        let block_class_id = builder
            .new_ruleset()
            .with_css_selector(css_selector!(".class_1"))
            .with_property(StyleBuilderProperty::new(
                TEST_PROPERTY,
                ReflectValue::Float(2f32),
            ))
            .block_id();

        let block_class_with_hover_id = builder
            .new_ruleset()
            .with_css_selector(css_selector!(".class_1:hover"))
            .with_property(StyleBuilderProperty::new(
                TEST_PROPERTY,
                ReflectValue::Float(4f32),
            ))
            .block_id();

        let block_any_id = builder
            .new_ruleset()
            .with_css_selector(css_selector!("*"))
            .with_property(StyleBuilderProperty::new(
                TEST_PROPERTY,
                ReflectValue::Float(0f32),
            ))
            .block_id();

        let mut style_blocks = Assets::<StyleBlock>::default();

        let mut custom_loader = TestAssetLoader(&mut style_blocks);

        let style_sheet = builder
            .build(
                &EMPTY_TYPE_REGISTRY,
                &PROPERTY_REGISTRY,
                StyleAssetLoader::custom(&mut custom_loader),
            )
            .unwrap();

        let media_provider = TestMediaProvider::default();

        let [
            block_with_name_id,
            block_class_id,
            block_class_with_hover_id,
            block_any_id,
        ] = translate_ids(
            &style_blocks,
            [
                block_with_name_id,
                block_class_id,
                block_class_with_hover_id,
                block_any_id,
            ],
        );

        assert_eq!(
            style_sheet.get_matching_block_ids_for_element(element!(Text), &media_provider),
            vec![block_any_id]
        );
        assert_eq!(
            style_sheet.get_matching_block_ids_for_element(element!(Text.class_1), &media_provider),
            vec![block_any_id, block_class_id]
        );
        assert_eq!(
            style_sheet.get_matching_block_ids_for_element(
                element!(text.class_1 #test_name),
                &media_provider
            ),
            vec![block_any_id, block_class_id, block_with_name_id]
        );
        assert_eq!(
            style_sheet.get_matching_block_ids_for_element(
                element!(text.class_1 :hover #test_name),
                &media_provider
            ),
            vec![
                block_any_id,
                block_class_id,
                block_class_with_hover_id,
                block_with_name_id
            ]
        );

        assert_eq!(
            get_properties!(style_blocks, [block_any_id, block_class_id]),
            property_map! { TEST_PROPERTY => ReflectValue::Float(2.0) }
        );

        assert_eq!(
            get_properties!(
                style_blocks,
                [
                    block_any_id,
                    block_class_id,
                    block_class_with_hover_id,
                    block_with_name_id
                ]
            ),
            property_map! { TEST_PROPERTY => ReflectValue::Float(4.0) }
        );

        let block_resolver = StyleResolver::new(
            &style_blocks,
            [block_any_id, block_class_id, block_class_with_hover_id],
        );

        assert_eq!(
            block_resolver.resolve_transition_options(
                &PROPERTY_REGISTRY,
                &CSS_PROPERTY_REGISTRY,
                &NoVarsSupportedResolver,
            ),
            properties! {}
        );

        let expected_transition_options = TransitionOptions {
            duration: Duration::from_secs(1),
            ..Default::default()
        };

        let block_resolver = StyleResolver::new(
            &style_blocks,
            [
                block_any_id,
                block_class_id,
                block_class_with_hover_id,
                block_with_name_id,
            ],
        );

        assert_eq!(
            block_resolver.resolve_transition_options(
                &PROPERTY_REGISTRY,
                &CSS_PROPERTY_REGISTRY,
                &NoVarsSupportedResolver,
            ),
            properties! { TEST_PROPERTY => expected_transition_options }
        );

        let block_resolver = StyleResolver::new(
            &style_blocks,
            [block_any_id, block_class_id, block_class_with_hover_id],
        );

        assert_eq!(
            block_resolver.resolve_animation_configs(&NoVarsSupportedResolver,),
            vec![]
        );

        let block_resolver = StyleResolver::new(
            &style_blocks,
            [block_any_id, block_class_id, block_with_name_id],
        );

        let resolved_animations =
            block_resolver.resolve_animation_configs(&NoVarsSupportedResolver);

        assert_eq!(
            resolved_animations,
            vec![AnimationConfiguration {
                name: TEST_ANIMATION_NAME.into(),
                default_timing_function: EasingFunction::default(),
                options: AnimationOptions {
                    duration: Duration::from_secs(1),
                    ..Default::default()
                },
            }]
        );
    }

    #[test]
    fn test_media_selectors() {
        let mut builder = StyleSheetBuilder::new();

        let any_selector = CssSelector::parse_single("*").unwrap();

        let block_dark = builder
            .new_ruleset()
            .with_css_selector(
                any_selector
                    .clone()
                    .with_media_selectors(MediaSelector::ColorScheme(ColorScheme::Dark)),
            )
            .with_property(StyleBuilderProperty::new(
                TEST_PROPERTY,
                ReflectValue::Float(0f32),
            ))
            .block_id();

        let block_light = builder
            .new_ruleset()
            .with_css_selector(
                any_selector
                    .clone()
                    .with_media_selectors(MediaSelector::ColorScheme(ColorScheme::Light)),
            )
            .with_property(StyleBuilderProperty::new(
                TEST_PROPERTY,
                ReflectValue::Float(1f32),
            ))
            .block_id();

        let block_min_width_500 = builder
            .new_ruleset()
            .with_css_selector(any_selector.clone().with_media_selectors([
                MediaSelector::ColorScheme(ColorScheme::Light),
                MediaSelector::ViewportWidth(MediaRangeSelector::GreaterOrEqual(500)),
            ]))
            .with_property(StyleBuilderProperty::new(
                TEST_PROPERTY,
                ReflectValue::Float(2f32),
            ))
            .block_id();

        let block_width_exact_600 = builder
            .new_ruleset()
            .with_css_selector(any_selector.clone().with_media_selectors([
                MediaSelector::ColorScheme(ColorScheme::Light),
                MediaSelector::ViewportWidth(MediaRangeSelector::Exact(600)),
            ]))
            .with_property(StyleBuilderProperty::new(
                TEST_PROPERTY,
                ReflectValue::Float(3f32),
            ))
            .block_id();

        let block_aspect_ratio_ge_1 = builder
            .new_ruleset()
            .with_css_selector(any_selector.clone().with_media_selectors(
                MediaSelector::AspectRatio(MediaRangeSelector::GreaterOrEqual(1.0)),
            ))
            .with_property(StyleBuilderProperty::new(
                TEST_PROPERTY,
                ReflectValue::Float(3f32),
            ))
            .block_id();

        let block_aspect_ratio_le_1 = builder
            .new_ruleset()
            .with_css_selector(any_selector.clone().with_media_selectors(
                MediaSelector::AspectRatio(MediaRangeSelector::LessOrEqual(1.0)),
            ))
            .with_property(StyleBuilderProperty::new(
                TEST_PROPERTY,
                ReflectValue::Float(3f32),
            ))
            .block_id();

        let mut style_blocks = Assets::<StyleBlock>::default();

        let mut custom_loader = TestAssetLoader(&mut style_blocks);

        let style_sheet = builder
            .build(
                &EMPTY_TYPE_REGISTRY,
                &PROPERTY_REGISTRY,
                StyleAssetLoader::custom(&mut custom_loader),
            )
            .unwrap();

        let [
            block_dark,
            block_light,
            block_min_width_500,
            block_width_exact_600,
            block_aspect_ratio_ge_1,
            block_aspect_ratio_le_1,
        ] = translate_ids(
            &style_blocks,
            [
                block_dark,
                block_light,
                block_min_width_500,
                block_width_exact_600,
                block_aspect_ratio_ge_1,
                block_aspect_ratio_le_1,
            ],
        );

        let dark_media = TestMediaProvider {
            scheme: Some(ColorScheme::Dark),
            ..Default::default()
        };

        assert_eq!(
            style_sheet.get_matching_block_ids_for_element(element!(Text), &dark_media),
            vec![block_dark]
        );

        let light_media = TestMediaProvider {
            scheme: Some(ColorScheme::Light),
            ..Default::default()
        };

        assert_eq!(
            style_sheet.get_matching_block_ids_for_element(element!(Text), &light_media),
            vec![block_light]
        );

        let light_500_media = TestMediaProvider {
            scheme: Some(ColorScheme::Light),
            width: Some(500),
            ..Default::default()
        };

        assert_eq!(
            style_sheet.get_matching_block_ids_for_element(element!(Text), &light_500_media),
            vec![block_light, block_min_width_500]
        );

        let light_600_media = TestMediaProvider {
            scheme: Some(ColorScheme::Light),
            width: Some(600),
            ..Default::default()
        };

        assert_eq!(
            style_sheet.get_matching_block_ids_for_element(element!(Text), &light_600_media),
            vec![block_light, block_min_width_500, block_width_exact_600]
        );

        let resolution_800x600 = TestMediaProvider {
            width: Some(800),
            height: Some(600),
            ..Default::default()
        };

        assert_eq!(
            style_sheet.get_matching_block_ids_for_element(element!(Text), &resolution_800x600),
            vec![block_aspect_ratio_ge_1]
        );

        let resolution_600x800 = TestMediaProvider {
            width: Some(600),
            height: Some(800),
            ..Default::default()
        };

        assert_eq!(
            style_sheet.get_matching_block_ids_for_element(element!(Text), &resolution_600x800),
            vec![block_aspect_ratio_le_1]
        );
    }

    #[test]
    fn extend_appends_rules() {
        let mut builder_a = StyleSheetBuilder::new();
        builder_a
            .new_ruleset()
            .with_css_selector(css_selector!(".only_a"))
            .with_property(StyleBuilderProperty::new(
                TEST_PROPERTY,
                ReflectValue::Float(1f32),
            ))
            .block_id();

        let mut builder_b = StyleSheetBuilder::new();
        builder_b
            .new_ruleset()
            .with_css_selector(css_selector!(".only_b"))
            .with_property(StyleBuilderProperty::new(
                TEST_PROPERTY,
                ReflectValue::Float(2f32),
            ))
            .block_id();

        let mut style_blocks = Assets::<StyleBlock>::default();

        let sheet_a = {
            let mut custom_loader = TestAssetLoader(&mut style_blocks);
            builder_a
                .build(
                    &EMPTY_TYPE_REGISTRY,
                    &PROPERTY_REGISTRY,
                    StyleAssetLoader::custom(&mut custom_loader),
                )
                .unwrap()
        };
        let sheet_b = {
            let mut custom_loader = TestAssetLoader(&mut style_blocks);
            builder_b
                .build(
                    &EMPTY_TYPE_REGISTRY,
                    &PROPERTY_REGISTRY,
                    StyleAssetLoader::custom(&mut custom_loader),
                )
                .unwrap()
        };

        let media_provider = TestMediaProvider::default();

        // Before the merge, `a` has no knowledge of `b`'s selector.
        assert!(
            sheet_a
                .get_matching_block_ids_for_element(element!(div.only_b), &media_provider)
                .is_empty(),
            "sheet_a should not match a selector that only exists in sheet_b"
        );

        let mut merged = sheet_a;
        merged.extend(sheet_b);

        // After the merge, the combined sheet matches both selectors.
        assert_eq!(
            merged
                .get_matching_block_ids_for_element(element!(div.only_a), &media_provider)
                .len(),
            1,
            "merged sheet should still match sheet_a's own selector"
        );
        assert_eq!(
            merged
                .get_matching_block_ids_for_element(element!(div.only_b), &media_provider)
                .len(),
            1,
            "merged sheet should now match sheet_b's selector too"
        );
    }

    /// Concatenating two individually-sorted `css_selectors_to_blocks` vectors
    /// is NOT a globally sorted vector; `extend` must re-sort by
    /// (layer, specificity) so a higher-specificity rule wins across the sheet
    /// boundary, not just within one operand. `self` carries the
    /// higher-specificity `#box.c` rule; `other` carries a lower-specificity
    /// `.c` rule on the same element. Even though `other` is merged in AFTER
    /// `self` (and would win a same-specificity tie, per `extend_appends_rules`
    /// above), `self`'s rule must still win here because it is MORE specific.
    #[test]
    fn extend_sorts_by_specificity_across_sheets() {
        let mut builder_self = StyleSheetBuilder::new();
        builder_self
            .new_ruleset()
            .with_css_selector(css_selector!("#box.c"))
            .with_property(StyleBuilderProperty::new(
                TEST_PROPERTY,
                ReflectValue::Float(50f32),
            ))
            .block_id();

        let mut builder_other = StyleSheetBuilder::new();
        builder_other
            .new_ruleset()
            .with_css_selector(css_selector!(".c"))
            .with_property(StyleBuilderProperty::new(
                TEST_PROPERTY,
                ReflectValue::Float(10f32),
            ))
            .block_id();

        let mut style_blocks = Assets::<StyleBlock>::default();

        let sheet_self = {
            let mut custom_loader = TestAssetLoader(&mut style_blocks);
            builder_self
                .build(
                    &EMPTY_TYPE_REGISTRY,
                    &PROPERTY_REGISTRY,
                    StyleAssetLoader::custom(&mut custom_loader),
                )
                .unwrap()
        };
        let sheet_other = {
            let mut custom_loader = TestAssetLoader(&mut style_blocks);
            builder_other
                .build(
                    &EMPTY_TYPE_REGISTRY,
                    &PROPERTY_REGISTRY,
                    StyleAssetLoader::custom(&mut custom_loader),
                )
                .unwrap()
        };

        let mut merged = sheet_self;
        merged.extend(sheet_other);

        let media_provider = TestMediaProvider::default();
        let ids = merged
            .get_matching_block_ids_for_element(element!(div #box .c), &media_provider);
        assert_eq!(ids.len(), 2, "both rules should match <div id=box class=c>");

        assert_eq!(
            get_properties!(style_blocks, ids),
            property_map! { TEST_PROPERTY => ReflectValue::Float(50.0) },
            "the higher-specificity #box.c rule from `self` must win over the \
             lower-specificity .c rule from `other`, even though `other` was \
             merged in after `self`"
        );
    }
}
