//! Horizontal nine-slice sprites for world-space UI (render layer 3).
//!
//! These menus draw `Text2d`, not Bevy UI nodes, so banners are [`Sprite`]s with
//! [`SpriteImageMode::Sliced`]. Height stays at the texture height so the end caps
//! are not stretched. Width grows to the label once text layout is ready.

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::sprite::{BorderRect, SliceScaleMode, SpriteImageMode, TextureSlicer};
use bevy::text::TextLayoutInfo;

use super::game_fonts::FontStyle;

/// Extra space on each side of the label, inside the stretchable center.
const BANNER_LABEL_PADDING_X: f32 = 6.0;
/// Local Y offset so the label sits in the ribbon band, above the hanging tails.
pub(crate) const BANNER_LABEL_OFFSET_Y: f32 = 4.0;

/// A texture sliced so it can grow wider than its source image.
#[derive(Clone, Copy)]
pub struct NineSliceDef {
    pub path: &'static str,
    pub native_size: Vec2,
    /// Insets in texture pixels. `min_inset` is left/top, `max_inset` is right/bottom.
    pub border: BorderRect,
}

impl NineSliceDef {
    pub fn slicer(self) -> TextureSlicer {
        TextureSlicer {
            border: self.border,
            center_scale_mode: SliceScaleMode::Stretch,
            sides_scale_mode: SliceScaleMode::Stretch,
            max_corner_scale: 1.0,
        }
    }

    pub fn sprite(self, asset_server: &AssetServer, size: Vec2) -> Sprite {
        Sprite {
            image: asset_server.load(self.path),
            custom_size: Some(size),
            image_mode: SpriteImageMode::Sliced(self.slicer()),
            ..default()
        }
    }
}

/// `assets/ui/BannerPurple.png` (70×40). Columns 22–48 are the flat middle.
pub const PURPLE_BANNER: NineSliceDef = NineSliceDef {
    path: "ui/BannerPurple.png",
    native_size: Vec2::new(70.0, 40.0),
    border: BorderRect {
        min_inset: Vec2::new(22.0, 0.0),
        max_inset: Vec2::new(21.0, 0.0),
    },
};

/// `assets/ui/BannerPurpleLarge.png` (84×60). Columns 33–51 are the flat middle.
pub const PURPLE_BANNER_LARGE: NineSliceDef = NineSliceDef {
    path: "ui/BannerPurpleLarge.png",
    native_size: Vec2::new(84.0, 60.0),
    border: BorderRect {
        min_inset: Vec2::new(33.0, 0.0),
        max_inset: Vec2::new(32.0, 0.0),
    },
};

/// `assets/ui/BannerBeigeLarge.png` (84×60). Same slice as [`PURPLE_BANNER_LARGE`].
pub const BEIGE_BANNER_LARGE: NineSliceDef = NineSliceDef {
    path: "ui/BannerBeigeLarge.png",
    native_size: Vec2::new(84.0, 60.0),
    border: BorderRect {
        min_inset: Vec2::new(33.0, 0.0),
        max_inset: Vec2::new(32.0, 0.0),
    },
};

/// Sliced sprite whose width tracks a [`NineSliceBannerText`] child.
#[derive(Component)]
pub struct FitNineSliceToText {
    pub min_size: Vec2,
    pub cap_left: f32,
    pub cap_right: f32,
    pub padding_x: f32,
}

/// Label drawn in front of a [`FitNineSliceToText`] banner.
#[derive(Component)]
pub struct NineSliceBannerText;

/// Spawns a banner at `translation` with `title` centered on top of it (local z = 1).
///
/// Width starts at the texture size and is updated by [`fit_nine_slice_banners_to_text`]
/// after the font has laid out. Insert [`crate::ui::UIState`] on the returned entity;
/// despawning it removes the label too.
pub fn spawn_growable_banner_title(
    commands: &mut Commands,
    asset_server: &AssetServer,
    slice: NineSliceDef,
    font: FontStyle,
    title: impl Into<String>,
    color: Color,
    translation: Vec3,
) -> Entity {
    let root = commands
        .spawn((
            slice.sprite(asset_server, slice.native_size),
            Transform::from_translation(translation),
            RenderLayers::from_layers(&[3]),
            FitNineSliceToText {
                min_size: slice.native_size,
                cap_left: slice.border.min_inset.x,
                cap_right: slice.border.max_inset.x,
                padding_x: BANNER_LABEL_PADDING_X,
            },
            Name::new("Nine Slice Banner"),
        ))
        .id();

    commands.spawn((
        font.text(asset_server, title, color)
            .with_transform(Transform::from_translation(Vec3::new(
                0.0,
                BANNER_LABEL_OFFSET_Y,
                1.0,
            ))),
        RenderLayers::from_layers(&[3]),
        NineSliceBannerText,
        ChildOf(root),
        Name::new("Nine Slice Banner Text"),
    ));

    root
}

/// Grows each fitted banner so the label sits in the stretchable center.
pub fn fit_nine_slice_banners_to_text(
    mut banners: Query<(&Children, &mut Sprite, &FitNineSliceToText)>,
    labels: Query<(&TextLayoutInfo, &Transform), With<NineSliceBannerText>>,
) {
    for (children, mut sprite, fit) in &mut banners {
        let Some((layout, transform)) = children.iter().find_map(|child| labels.get(child).ok())
        else {
            continue;
        };
        if layout.size.x <= 0.0 {
            continue;
        }

        // `TextLayoutInfo::size` is already in world units. Font role scale is on the label.
        let text_width = layout.size.x * transform.scale.x;
        let width = (text_width + fit.cap_left + fit.cap_right + fit.padding_x * 2.0)
            .ceil()
            .max(fit.min_size.x);
        let size = Vec2::new(width, fit.min_size.y);
        if sprite.custom_size != Some(size) {
            sprite.custom_size = Some(size);
        }
    }
}
