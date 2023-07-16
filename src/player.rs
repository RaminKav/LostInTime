use bevy::{prelude::*, sprite::MaterialMesh2dBundle};
use bevy_ecs_tilemap::tiles::TilePos;
use bevy_rapier2d::prelude::{
    ActiveEvents, CharacterLength, Collider, KinematicCharacterController,
    KinematicCharacterControllerOutput, QueryFilterFlags, RigidBody,
};
use serde::Deserialize;
use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter};

use crate::{
    animations::{AnimatedTextureMaterial, AnimationFrameTracker, AnimationTimer},
    attributes::{
        Attack, AttackCooldown, InvincibilityCooldown, ItemAttributes, MaxHealth,
        PlayerAttributeBundle,
    },
    inputs::{FacingDirection, InputsPlugin, MovementVector},
    inventory::{Inventory, INVENTORY_INIT, INVENTORY_SIZE},
    item::EquipmentData,
    world::{y_sort::YSort, CHUNK_SIZE},
    AppExt, CoreGameSet, Game, GameParam, RawPosition,
};
pub struct PlayerPlugin;

pub struct MovePlayerEvent {
    pub chunk_pos: IVec2,
    pub tile_pos: TilePos,
}
#[derive(Component, Debug)]
pub struct Player;
#[derive(Debug, Clone)]
pub struct PlayerState {
    pub direction: FacingDirection,
    pub is_moving: bool,
    pub is_dashing: bool,
    pub is_attacking: bool,
    pub main_hand_slot: Option<EquipmentData>,
    pub position: Vec3,
    pub reach_distance: u8,
    pub player_dash_cooldown: Timer,
    pub player_dash_duration: Timer,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            direction: FacingDirection::Left,
            is_moving: false,
            is_dashing: false,
            is_attacking: false,
            main_hand_slot: None,
            position: Vec3::ZERO,
            reach_distance: 1,
            player_dash_cooldown: Timer::from_seconds(0.5, TimerMode::Once),
            player_dash_duration: Timer::from_seconds(0.1, TimerMode::Once),
        }
    }
}
#[derive(Component, EnumIter, Display, Debug, Hash, Copy, Clone, PartialEq, Eq, Deserialize)]
pub enum Limb {
    Torso,
    Hands,
    Legs,
    Head,
}
impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.with_default_schedule(CoreSchedule::FixedUpdate, |app| {
            app.add_event::<MovePlayerEvent>();
        })
        .add_startup_system(spawn_player)
        .add_system(test_swap_armor_texture)
        .add_system(
            handle_move_player
                .in_set(CoreGameSet::Main)
                .in_schedule(CoreSchedule::FixedUpdate),
        )
        .add_system(
            handle_player_raw_position
                .before(InputsPlugin::move_player)
                .in_set(CoreGameSet::Main)
                .in_schedule(CoreSchedule::FixedUpdate),
        );
    }
}
pub fn handle_move_player(
    mut player: Query<(&mut RawPosition, &mut Transform), With<Player>>,
    mut move_events: EventReader<MovePlayerEvent>,
) {
    for m in move_events.iter() {
        //TODO: Add world helper to get chunk -> world pos, lots of copy code in item.rs
        let new_pos = Vec3::new(
            (m.tile_pos.x as i32 * 32 + m.chunk_pos.x * CHUNK_SIZE as i32 * 32) as f32,
            (m.tile_pos.y as i32 * 32 + m.chunk_pos.y * CHUNK_SIZE as i32 * 32) as f32,
            0.,
        );
        let (mut raw_pos, mut pos) = player.single_mut();
        raw_pos.0 = new_pos.truncate();
        pos.translation = new_pos;
    }
}
/// Updates the player's [RawPosition] based on the [KinematicCharacterControllerOutput]
/// we store the un-rounded raw position, and then round the [Transform] position.
pub fn handle_player_raw_position(
    mut player_pos: Query<
        (
            &mut RawPosition,
            &mut Transform,
            &KinematicCharacterControllerOutput,
        ),
        (With<Player>, Changed<KinematicCharacterControllerOutput>),
    >,
    mut game: GameParam,
) {
    let Ok((mut raw_pos, mut pos,  kcc)) = player_pos.get_single_mut() else {return};
    raw_pos.0 += kcc.effective_translation;

    let delta = raw_pos.0 - pos.translation.truncate();

    pos.translation.x += delta.x;
    pos.translation.y += delta.y;
    pos.translation = pos.translation.round();
    game.game.player_state.position = pos.translation;
}

fn spawn_player(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlases: ResMut<Assets<TextureAtlas>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<AnimatedTextureMaterial>>,

    mut game: ResMut<Game>,
    _images: ResMut<Assets<Image>>,
) {
    let mut limb_children: Vec<Entity> = vec![];
    //player shadow
    let shadow_texture_handle = asset_server.load("textures/player/player-shadow.png");
    let shadow_texture_atlas =
        TextureAtlas::from_grid(shadow_texture_handle, Vec2::new(32., 32.), 1, 1, None, None);
    let shadow_texture_atlas_handle = texture_atlases.add(shadow_texture_atlas);

    let shadow = commands
        .spawn(SpriteSheetBundle {
            texture_atlas: shadow_texture_atlas_handle,
            transform: Transform::from_translation(Vec3::new(0., 0., -0.00000001)),
            ..default()
        })
        .id();
    limb_children.push(shadow);

    //player
    for l in Limb::iter() {
        let limb_source_handle = asset_server.load(format!(
            "textures/player/player-run-down/player-{}-run-down-source-0.png",
            l.to_string().to_lowercase()
        ));

        let limb_texture_asset = format!(
            "textures/player/player-texture-{}.png",
            if l == Limb::Torso || l == Limb::Hands {
                Limb::Torso.to_string().to_lowercase()
            } else {
                l.to_string().to_lowercase()
            }
        );
        let limb_texture_handle = asset_server.load(limb_texture_asset);
        // let limb_texture_atlas =
        //     TextureAtlas::from_grid(limb_texture_handle, Vec2::new(32., 32.), 5, 1, None, None);

        // let limb_texture_atlas_handle = texture_atlases.add(limb_texture_atlas);
        let transform = if l == Limb::Head {
            Transform::from_translation(Vec3::new(0., 0., 0.))
        } else {
            Transform::default()
        };
        let limb = commands
            .spawn((
                MaterialMesh2dBundle {
                    mesh: meshes
                        .add(
                            shape::Quad {
                                size: Vec2::new(32., 32.),
                                ..Default::default()
                            }
                            .into(),
                        )
                        .into(),
                    transform,
                    material: materials.add(AnimatedTextureMaterial {
                        source_texture: Some(limb_source_handle),
                        lookup_texture: Some(limb_texture_handle),
                        opacity: 1.,
                        flip: 1.,
                    }),
                    ..default()
                },
                l,
                AnimationFrameTracker(0, 5),
            ))
            .id();
        // .spawn(SpriteSheetBundle {
        //     texture_atlas: limb_texture_atlas_handle,
        //     transform,
        //     ..default()
        // })
        // .id();
        limb_children.push(limb);
    }

    //spawn player entity with limb spritesheets as children
    let p = commands
        .spawn((
            SpatialBundle {
                transform: Transform::from_translation(Vec3::new(0., 0., 1.)),
                ..Default::default()
            },
            AnimationTimer(Timer::from_seconds(0.25, TimerMode::Repeating)),
            Player,
            Inventory {
                items: [INVENTORY_INIT; INVENTORY_SIZE],
                crafting_items: [INVENTORY_INIT; 4],
                crafting_result_item: None,
                equipment_items: [INVENTORY_INIT; 4],
                accessory_items: [INVENTORY_INIT; 4],
            },
            ItemAttributes {
                health: 100,
                attack: 5,
                ..default()
            },
            PlayerAttributeBundle {
                health: MaxHealth(100),
                attack: Attack(5),
                attack_cooldown: AttackCooldown(0.4),
            },
            InvincibilityCooldown(1.),
            MovementVector::default(),
            YSort,
            Name::new("Player"),
            Collider::cuboid(7., 5.),
            KinematicCharacterController {
                // The character offset is set to 0.01.
                offset: CharacterLength::Absolute(0.01),
                filter_flags: QueryFilterFlags::EXCLUDE_SENSORS,
                ..default()
            },
            RawPosition::default(),
        ))
        .insert(ActiveEvents::COLLISION_EVENTS)
        .insert(RigidBody::KinematicPositionBased)
        .push_children(&limb_children)
        .id();
    game.player = p;
}
pub fn test_swap_armor_texture(
    player_limbs: Query<(&mut Handle<AnimatedTextureMaterial>, &Limb)>,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<AnimatedTextureMaterial>>,
    keys: Res<Input<KeyCode>>,
) {
    if keys.just_pressed(KeyCode::F) {
        for (mat, limb) in player_limbs.iter() {
            if limb == &Limb::Torso || limb == &Limb::Hands {
                let mut mat = materials.get_mut(mat).unwrap();
                let armor_texture_handle = asset_server.load("textures/player/armor-torso.png");
                mat.lookup_texture = Some(armor_texture_handle);
            }
        }
    }
}
