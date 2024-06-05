use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};

use crate::{
    assets::Graphics,
    colors::BLACK,
    player::stats::{PlayerStats, SkillPoints},
    GAME_HEIGHT, GAME_WIDTH,
};

use super::{Interactable, ShowInvPlayerStatsEvent, UIElement, UIState, STATS_UI_SIZE};

#[derive(Component)]
pub struct StatsUI;

#[derive(Component)]
pub struct StatsText {
    pub index: usize,
}
#[derive(Component)]
pub struct SPText;

#[derive(Component)]
pub struct StatsButtonState {
    pub index: usize,
}

pub fn setup_stats_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    mut stats_event: EventWriter<ShowInvPlayerStatsEvent>,
    asset_server: Res<AssetServer>,
    player_stats: Query<(&PlayerStats, &SkillPoints)>,
) {
    let (size, texture, t_offset) = (
        STATS_UI_SIZE,
        graphics.get_ui_element_texture(UIElement::LevelUpStats),
        Vec2::new(3.5, 4.),
    );

    let overlay = commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: Color::rgba(146. / 255., 116. / 255., 65. / 255., 0.3),
                custom_size: Some(Vec2::new(GAME_WIDTH + 10., GAME_HEIGHT + 10.)),
                ..default()
            },
            transform: Transform {
                translation: Vec3::new(-t_offset.x, -t_offset.y, -1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("overlay"))
        .id();
    let stats_e = commands
        .spawn(SpriteBundle {
            texture,
            sprite: Sprite {
                custom_size: Some(size),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::new(t_offset.x, t_offset.y, 10.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(StatsUI)
        .insert(Name::new("STATS UI"))
        .insert(UIState::Stats)
        .insert(RenderLayers::from_layers(&[3]))
        .id();

    for i in -1..3 {
        let translation = Vec3::new(6., (-i as f32 * 21.) + 10., 1.);
        let mut slot_entity = commands.spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::StatsButton),
            transform: Transform {
                translation,
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            sprite: Sprite {
                custom_size: Some(Vec2::new(16., 16.)),
                ..Default::default()
            },
            ..Default::default()
        });
        slot_entity
            .set_parent(stats_e)
            .insert(Interactable::default())
            .insert(UIElement::StatsButton)
            .insert(StatsButtonState {
                index: i as usize + 1,
            })
            .insert(RenderLayers::from_layers(&[3]))
            .insert(Name::new("STATS BUTTON"));

        let mut text = commands.spawn((
            Text2dBundle {
                text: Text::from_section(
                    player_stats
                        .single()
                        .0
                        .get_stats_from_ui_index(i + 1)
                        .to_string(),
                    TextStyle {
                        font: asset_server.load("fonts/Kitchen Sink.ttf"),
                        font_size: 8.0,
                        color: BLACK,
                    },
                ),
                text_anchor: Anchor::CenterLeft,
                transform: Transform {
                    translation: Vec3::new(23., (-i as f32 * 21.) + 10., 1.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            },
            Name::new("STATS TEXT"),
            RenderLayers::from_layers(&[3]),
        ));
        text.set_parent(stats_e)
            .insert(StatsText {
                index: i as usize + 1,
            })
            .insert(RenderLayers::from_layers(&[3]));
    }

    // sp remaining text
    let mut sp_text = commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                player_stats.single().1.count.to_string(),
                TextStyle {
                    font: asset_server.load("fonts/Kitchen Sink.ttf"),
                    font_size: 8.0,
                    color: BLACK,
                },
            ),
            text_anchor: Anchor::CenterLeft,
            transform: Transform {
                translation: Vec3::new(29., 43., 1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        },
        Name::new("SP TEXT"),
        RenderLayers::from_layers(&[3]),
    ));
    sp_text
        .set_parent(stats_e)
        .insert(SPText)
        .insert(RenderLayers::from_layers(&[3]));

    commands.entity(stats_e).push_children(&[overlay]);

    stats_event.send(ShowInvPlayerStatsEvent);
}

pub fn toggle_stats_visibility(
    mut next_inv_state: ResMut<NextState<UIState>>,
    key_input: ResMut<Input<KeyCode>>,
) {
    if key_input.just_pressed(KeyCode::B) {
        next_inv_state.set(UIState::Stats);
    }
}

pub fn update_stats_text(
    mut stats_text_query: Query<(&mut Text, &StatsText)>,
    player_stats: Query<&PlayerStats, Changed<PlayerStats>>,
) {
    let Ok(stats) = player_stats.get_single() else {
        return;
    };
    for (mut text, stats_text) in stats_text_query.iter_mut() {
        text.sections[0].value = stats
            .get_stats_from_ui_index(stats_text.index as i32)
            .to_string();
    }
}

pub fn update_sp_text(
    mut sp_text_query: Query<(&mut Text, &SPText)>,
    player_stats: Query<&SkillPoints, Changed<SkillPoints>>,
) {
    let Ok(sp) = player_stats.get_single() else {
        return;
    };
    for (mut text, _) in sp_text_query.iter_mut() {
        text.sections[0].value = sp.count.to_string();
    }
}
