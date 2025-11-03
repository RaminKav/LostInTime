use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};
use rand::Rng;

use crate::{
    assets::Graphics,
    attributes::{Attack, BonusDamage, CurrentHealth, MaxHealth},
    colors::{BLACK, DMG_NUM_GREEN, DMG_NUM_PURPLE, DMG_NUM_RED, DMG_NUM_YELLOW},
    inventory::ItemStack,
    item::WorldObject,
    world::{world_helpers, TILE_SIZE},
    Game, TextureCamera, WasHitWithCrit,
};

use super::{spawn_item_stack_icon, UIElement};

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

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeaconTarget {
    Portal,
    DungeonEntrance,
    BossShrine,
}

#[derive(Component)]
pub struct BeaconGuidance(pub BeaconTarget);

#[derive(Resource, Default)]
pub struct BeaconGuidanceRegistry {
    pub portal: Option<Entity>,
    pub dungeon: Option<Entity>,
    pub boss: Option<Entity>,
}

#[derive(Component)]
pub struct ScreenLockedTargetWorldPos(pub Vec2);

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
        (
            Entity,
            &CurrentHealth,
            &mut PreviousHealth,
            Option<&WasHitWithCrit>,
        ),
        Changed<CurrentHealth>,
    >,
    txfms: Query<&GlobalTransform>,
    asset_server: Res<AssetServer>,
    raw_dmg: Query<(&Attack, &BonusDamage)>,
    game: Res<Game>,
) {
    for (e, changed_health, mut prev_health, crit_option) in changed_health.iter_mut() {
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
        let is_crit = crit_option.is_some() || (!is_player && delta.abs() > dmg && dmg != 0);

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
        if is_crit {
            commands.entity(e).remove::<WasHitWithCrit>();
        }
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

pub fn spawn_screen_locked_icon_to_world_pos(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    obj: WorldObject,
    world_pos: Vec2,
) -> Entity {
    let item_icon = spawn_item_stack_icon(
        commands,
        graphics,
        &ItemStack::crate_icon_stack(obj),
        asset_server,
        Vec2::ZERO,
        Vec2::new(0., 0.),
        3,
    );
    commands
        .entity(item_icon)
        .insert(Name::new("SCREEN ICON ITEM"));

    let mut binding = commands.spawn(SpriteBundle {
        texture: graphics.get_ui_element_texture(UIElement::ScreenIconSlotLarge),
        transform: Transform::default(), // Position will be set in handle_clamp_screen_locked_icons_worldpos
        sprite: Sprite {
            custom_size: Some(Vec2::new(20., 20.)),
            ..Default::default()
        },
        ..Default::default()
    });
    let slot_entity = binding
        .insert(RenderLayers::from_layers(&[3]))
        .insert(ScreenLockedTargetWorldPos(world_pos))
        .insert(Name::new("SCREEN ICON (WORLD POS)"))
        .push_children(&[item_icon]);
    slot_entity.id()
}

pub fn handle_clamp_screen_locked_icons_worldpos(
    mut query: Query<(&ScreenLockedTargetWorldPos, &mut Transform, &mut Visibility)>,
    game_camera: Query<&GlobalTransform, With<TextureCamera>>,
) {
    let MAX_DIST: Vec2 = Vec2::new(11.5, 7.) * TILE_SIZE.x - Vec2::new(0., 0.);
    let offset = Vec2::new(6., 5.);

    let camera_txfm = match game_camera.get_single() {
        Ok(t) => t,
        Err(_) => return,
    };

    let camera_pos = camera_txfm.translation().truncate();

    for (target, mut icon_txfm, mut v) in query.iter_mut() {
        // Vector from camera (UI origin) to target in UI space
        let ui_vec = world_helpers::world_pos_to_ui_screen_pos(target.0, camera_pos);

        // Half extents with padding
        let rx = MAX_DIST.x - offset.x;
        let ry = MAX_DIST.y - offset.y;

        // If the target is within the visible region, hide the icon
        if ui_vec.x.abs() <= rx && ui_vec.y.abs() <= ry {
            *v = Visibility::Hidden;
            continue;
        }

        if ui_vec.x == 0. && ui_vec.y == 0. {
            *v = Visibility::Hidden;
            continue;
        }

        // Project to the edge of the rect: scale vector so it hits the border
        let tx = rx / ui_vec.x.abs();
        let ty = ry / ui_vec.y.abs();
        let t = tx.min(ty);
        let edge = ui_vec * t;

        icon_txfm.translation = edge.extend(20.);
        *v = Visibility::Visible;
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
            if i == 0 {
                Vec3::new(1., -1., -1.)
            } else {
                pos + Vec3::ZERO
            },
            if i == 0 { BLACK } else { color },
            text.clone(),
            Anchor::Center,
            1.0,
            0,
        );
        if i == 0 {
            shadow_e = entity;
        } else {
            commands
                .entity(entity)
                .insert(DamageNumber {
                    timer: Timer::from_seconds(0.85, TimerMode::Once),
                    velocity: 0.,
                })
                .add_child(shadow_e);
        }
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
