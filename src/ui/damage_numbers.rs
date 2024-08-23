use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};
use rand::Rng;

use crate::{
    assets::Graphics,
    attributes::{Attack, BonusDamage, CurrentHealth, MaxHealth},
    colors::{BLACK, DMG_NUM_GREEN, DMG_NUM_PURPLE, DMG_NUM_RED, DMG_NUM_YELLOW},
    inventory::ItemStack,
    item::WorldObject,
    world::TILE_SIZE,
    Game, TextureCamera,
};

use super::{spawn_item_stack_icon, UIElement, UI_SLOT_SIZE};

#[derive(Component)]
pub struct DamageNumber {
    pub timer: Timer,
    pub velocity: f32,
}

#[derive(Component)]
pub struct PreviousHealth(pub i32);

pub struct DodgeEvent {
    pub entity: Entity,
}
#[derive(Component)]
pub struct ScreenLockedIcon {
    parent: Entity,
}

#[derive(Resource)]
pub struct NewRecipeTextTimer {
    pub timer: Timer,
    pub queue: Vec<WorldObject>,
}
impl NewRecipeTextTimer {
    pub fn new(secs: f32) -> Self {
        NewRecipeTextTimer {
            timer: Timer::from_seconds(secs, TimerMode::Once),
            queue: Vec::new(),
        }
    }
}

pub fn add_previous_health(
    mut commands: Commands,
    query: Query<(Entity, &MaxHealth), (Added<MaxHealth>, Without<PreviousHealth>)>,
) {
    for (entity, max_health) in query.iter() {
        commands.entity(entity).insert(PreviousHealth(max_health.0));
    }
}
// a function that adds damage numbers to the screen in response to a [HitEvent].
// the damage numbers are [Text2DBundle]s with a [DamageNumber] component.
// the [DamageNumber] component is used to delete the damage number after a short delay.

pub fn handle_add_damage_numbers_after_hit(
    mut commands: Commands,
    mut changed_health: Query<
        (Entity, &CurrentHealth, &mut PreviousHealth),
        Changed<CurrentHealth>,
    >,
    txfms: Query<&GlobalTransform>,
    asset_server: Res<AssetServer>,
    raw_dmg: Query<(&Attack, &BonusDamage)>,
    game: Res<Game>,
) {
    for (e, changed_health, mut prev_health) in changed_health.iter_mut() {
        let delta = changed_health.0 - prev_health.0;
        if delta == 0 {
            continue;
        }
        let mut rng = rand::thread_rng();
        let drop_spread = 16.;
        let pos_offset = Vec3::new(
            rng.gen_range(-drop_spread..drop_spread) as f32,
            rng.gen_range(0_f64..drop_spread) as f32,
            2.,
        );
        prev_health.0 = changed_health.0;
        let is_player = e == game.player;
        let dmg = raw_dmg.get(game.player).unwrap().0 .0 + raw_dmg.get(game.player).unwrap().1 .0;
        let is_crit = !is_player && delta.abs() > dmg && dmg != 0;
        spawn_floating_text_with_shadow(
            &mut commands,
            &asset_server,
            txfms.get(e).unwrap().translation() + pos_offset,
            if delta > 0 {
                DMG_NUM_GREEN
            } else if is_player {
                DMG_NUM_PURPLE
            } else if is_crit {
                DMG_NUM_YELLOW
            } else {
                DMG_NUM_RED
            },
            if delta < 0 {
                format!("{}{}", delta.abs(), if is_crit { "!" } else { "" })
            } else {
                format!("+{}", delta)
            },
        );
    }
}
pub fn handle_add_dodge_text(
    mut commands: Commands,
    mut dodge_events: EventReader<DodgeEvent>,
    txfms: Query<&GlobalTransform>,
    asset_server: Res<AssetServer>,
) {
    for event in dodge_events.iter() {
        let mut rng = rand::thread_rng();
        let drop_spread = 16.;
        let pos_offset = Vec3::new(
            i32::max(5 + rng.gen_range(-drop_spread..drop_spread) as i32, 10) as f32,
            i32::max(5 + rng.gen_range(0_f64..drop_spread) as i32, 10) as f32,
            2.,
        );
        spawn_floating_text_with_shadow(
            &mut commands,
            &asset_server,
            txfms.get(event.entity).unwrap().translation() + pos_offset,
            DMG_NUM_YELLOW,
            "Dodge!".to_string(),
        );
    }
}

// function that ticks the [DamageNumber] timer and deletes the [DamageNumber] when the timer is done.
pub fn tick_damage_numbers(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut Text, &mut DamageNumber, &mut Transform)>,
) {
    for (entity, mut text, mut damage_number, mut t) in query.iter_mut() {
        damage_number.timer.tick(time.delta());
        if damage_number.timer.percent() > 0.3 {
            t.translation.y += damage_number.velocity * time.delta_seconds();
            for section in text.sections.iter_mut() {
                section
                    .style
                    .color
                    .set_a(1. - damage_number.timer.percent() + 0.3);
            }
        }
        damage_number.velocity += 0.7;
        if damage_number.timer.finished() {
            commands.entity(entity).despawn_recursive();
        }
    }
}

pub fn spawn_screen_locked_icon(
    parent: Entity,
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    obj: WorldObject,
) {
    let item_icon = spawn_item_stack_icon(
        commands,
        graphics,
        &ItemStack::crate_icon_stack(obj),
        asset_server,
        Vec2::ZERO,
        Vec2::new(0., 0.),
        0,
    );
    commands
        .entity(item_icon)
        .insert(Name::new("SCREEN ICON ITEM"));

    let mut slot_entity = commands.spawn(SpriteBundle {
        texture: graphics.get_ui_element_texture(UIElement::ScreenIconSlot),
        transform: Transform::default(),
        sprite: Sprite {
            custom_size: Some(Vec2::new(UI_SLOT_SIZE, UI_SLOT_SIZE)),
            ..Default::default()
        },
        ..Default::default()
    });
    slot_entity
        .insert(ScreenLockedIcon { parent })
        .insert(Name::new("SCREEN ICON"));

    slot_entity.push_children(&[item_icon]);
}

pub fn handle_clamp_screen_locked_icons(
    mut commands: Commands,
    mut query: Query<(Entity, &ScreenLockedIcon, &mut Transform, &mut Visibility)>,
    txfms: Query<&GlobalTransform>,
    game_camera: Query<&GlobalTransform, With<TextureCamera>>,
) {
    let MAX_DIST: Vec2 = Vec2::new(11.5, 7.) * TILE_SIZE.x - Vec2::new(2., 1.);

    for (e, screen_locked_icon, mut icon_txfm, mut v) in query.iter_mut() {
        if let Ok(parent_txfm) = txfms.get(screen_locked_icon.parent) {
            icon_txfm.translation = parent_txfm.translation() + Vec3::new(0., 20., 1.);
            let camera_txfm = game_camera.single();

            let cx = camera_txfm.translation().x;
            let cy = camera_txfm.translation().y;

            if icon_txfm.translation.x > cx + MAX_DIST.x {
                icon_txfm.translation.x = cx + MAX_DIST.x;
            } else if icon_txfm.translation.x < cx - MAX_DIST.x {
                icon_txfm.translation.x = cx - MAX_DIST.x;
            }
            if icon_txfm.translation.y > cy + MAX_DIST.y {
                icon_txfm.translation.y = cy + MAX_DIST.y;
            } else if icon_txfm.translation.y < cy - MAX_DIST.y {
                icon_txfm.translation.y = cy - MAX_DIST.y;
            }

            // TOGGLE VISIBILITY WHEN PARENT IN VIEW
            if icon_txfm.translation.x < cx + MAX_DIST.x
                && icon_txfm.translation.x > cx - MAX_DIST.x
                && icon_txfm.translation.y < cy + MAX_DIST.y
                && icon_txfm.translation.y > cy - MAX_DIST.y
            {
                *v = Visibility::Hidden;
            } else {
                *v = Visibility::Visible;
            }
        } else if commands.get_entity(screen_locked_icon.parent).is_none() {
            commands.entity(e).despawn_recursive();
        }
    }
}

pub fn spawn_floating_text_with_shadow(
    commands: &mut Commands,
    asset_server: &AssetServer,
    pos: Vec3,
    color: Color,
    text: String,
) -> Entity {
    let mut shadow_e = Entity::from_raw(0);
    for i in 0..2 {
        let entity = spawn_text(
            commands,
            asset_server,
            pos + if i == 0 {
                Vec3::new(1., -1., -1.)
            } else {
                Vec3::ZERO
            },
            if i == 0 { BLACK } else { color },
            text.clone(),
            Anchor::Center,
            1.0,
            0,
        );
        if i == 0 {
            shadow_e = entity;
        }
        commands.entity(entity).insert(DamageNumber {
            timer: Timer::from_seconds(0.85, TimerMode::Once),
            velocity: 0.,
        });
    }
    shadow_e
}

pub fn spawn_text(
    commands: &mut Commands,
    asset_server: &AssetServer,
    pos: Vec3,
    color: Color,
    text: String,
    anchor: Anchor,
    font_scale: f32,
    render_layer: u8,
) -> Entity {
    commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                text,
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.0 * font_scale,
                    color,
                },
            ),
            transform: Transform {
                translation: pos,
                ..Default::default()
            },
            text_anchor: anchor,
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[render_layer]))
        .id()
}
