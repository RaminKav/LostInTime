use crate::{
    animations::AnimationTimer,
    assets::{SpriteAnchor, SpriteSize},
    attributes::{add_item_glows, ItemLevel, RawItemBaseAttributes},
    defs::{
        registry::GameDefs,
        spawn::{spawn_from_def, PendingSpriteSheet, PendingSpriteTexture},
    },
    inventory::ItemStack,
    item::{
        projectile::{ArcProjectileData, Projectile},
        EquipmentType, ItemDrop, Wall, WorldObject,
    },
    player::mage_skills::Electricity,
    proto::proto_param::ProtoParam,
    world::{
        wall_auto_tile::Dirty,
        world_helpers::{tile_pos_to_world_pos, world_pos_to_chunk_relative_tile_pos},
        WallTextureData,
    },
};
use bevy::prelude::*;
use bevy_aseprite::{anim::AsepriteAnimation, Aseprite};
use bevy_rapier2d::prelude::{ActiveCollisionTypes, ActiveEvents, Collider, Sensor};
use core::fmt::Display;
use std::f32::consts::PI;

pub trait CommandsExt {
    fn spawn_item_from_proto<'a, T: Display + Clone + Into<&'a str>>(
        &mut self,
        obj: T,
        params: &ProtoParam,
        pos: Vec2,
        count: usize,
        level: Option<u8>,
    ) -> Option<Entity>;
    fn spawn_projectile_from_proto<'a, T: Display + Clone + Into<&'a str>>(
        &mut self,
        obj: T,
        params: &ProtoParam,
        pos: Vec2,
        dir: Vec2,
        mana_bar_full: bool,
        asset_server: &AssetServer,
        scale_up: f32,
    ) -> Option<Entity>;
    fn spawn_from_proto<'a, T: Display + Clone + Into<&'a str>>(
        &mut self,
        obj: T,
        defs: &GameDefs,
        pos: Vec2,
    ) -> Option<Entity>;
    fn spawn_object_from_proto<'a, T: Display + Clone + Into<&'a str>>(
        &mut self,
        obj: T,
        pos: Vec2,
        proto_param: &ProtoParam,
        is_dirty: bool,
    ) -> Option<Entity>;
}

impl CommandsExt for Commands<'_, '_> {
    fn spawn_item_from_proto<'a, T: Display + Clone + Into<&'a str>>(
        &mut self,
        obj: T,
        params: &ProtoParam,
        pos: Vec2,
        count: usize,
        level: Option<u8>,
    ) -> Option<Entity> {
        if let Some(spawned_entity) = self.spawn_from_proto(obj.clone(), &params.defs, pos) {
            let Some(mut spawned_entity_commands) = self.get_entity(spawned_entity) else {
                return None;
            };

            let mut glow_rarity = None;
            if let Some(proto_data) = params.get_item_data(obj.clone()) {
                let mut proto_data = proto_data.clone();
                proto_data.count = count;
                if params
                    .get_component::<RawItemBaseAttributes, _>(obj.clone())
                    .is_none()
                {
                    glow_rarity = Some(proto_data.rarity.clone());
                }
                spawned_entity_commands.insert(proto_data).insert(ItemDrop);
                spawned_entity_commands.insert(crate::item::ItemDropDespawnTimer(
                    Timer::from_seconds(300.0, TimerMode::Once),
                ));
                let eqp_type = params
                    .get_component::<EquipmentType, _>(obj.clone())
                    .unwrap_or(&EquipmentType::None);
                if let Some(level) = level {
                    if eqp_type.is_weapon() || (eqp_type.is_equipment() && !eqp_type.is_accessory())
                    {
                        spawned_entity_commands.insert(ItemLevel(level));
                    }
                }
            }

            if let Some(sprite_map) = &params.graphics.spritesheet_map {
                if let Some(obj_type) = params.get_component::<WorldObject, _>(obj.clone()) {
                    if let Some(sprite) = sprite_map.get(obj_type) {
                        // Full SpriteSheetBundle (not bare atlas+sprite) so GlobalTransform
                        // is always present even if spawn_from_def regresses.
                        spawned_entity_commands
                            .insert(SpriteSheetBundle {
                                texture_atlas: params
                                    .graphics
                                    .texture_atlas
                                    .as_ref()
                                    .unwrap()
                                    .clone(),
                                sprite: sprite.clone(),
                                transform: Transform::from_translation(pos.extend(0.)),
                                ..default()
                            })
                            .remove::<PendingSpriteSheet>()
                            .remove::<PendingSpriteTexture>();
                    }
                }
            }

            if let Some(rarity) = glow_rarity {
                add_item_glows(self, &params.graphics, spawned_entity, rarity);
            }

            return Some(spawned_entity);
        }
        None
    }

    fn spawn_projectile_from_proto<'a, T: Display + Clone + Into<&'a str>>(
        &mut self,
        obj: T,
        params: &ProtoParam,
        pos: Vec2,
        dir: Vec2,
        mana_bar_full: bool,
        asset_server: &AssetServer,
        scale_up: f32,
    ) -> Option<Entity> {
        if let Some(spawned_entity) = self.spawn_from_proto(obj.clone(), &params.defs, pos) {
            let Some(mut spawned_entity_commands) = self.get_entity(spawned_entity) else {
                return None;
            };

            let Some(proj_state) = params.get_projectile_state(obj.clone()) else {
                return None;
            };
            let mut proto_data = proj_state.clone();
            proto_data.direction = dir;
            let sprite_size = if let Some(sprite_data) = params.get_sprite_sheet_data(obj.clone()) {
                sprite_data.size
            } else {
                Vec2::new(16., 16.)
            } * scale_up;
            let mut x_offset = 0.;
            let mut y_offset = 0.;
            let angle = proto_data.direction.y.atan2(proto_data.direction.x);
            if dir != Vec2::ZERO {
                x_offset = (angle.cos() * (sprite_size.x) + angle.cos() * (sprite_size.y)) / 2.;
                y_offset = (angle.sin() * (sprite_size.x) + angle.sin() * (sprite_size.y)) / 2.;
            }
            let custom_rotation =
                if let Some(proj) = params.get_component::<Projectile, _>(obj.clone()) {
                    proj.get_custom_rotation()
                } else {
                    None
                };
            proto_data.mana_bar_full = mana_bar_full;
            spawned_entity_commands
                .insert(proto_data)
                .insert(TransformBundle::from_transform(Transform {
                    translation: pos.extend(0.)
                        + Vec3::new(
                            x_offset + (angle.cos() * proj_state.spawn_offset.x * scale_up),
                            y_offset + (angle.sin() * proj_state.spawn_offset.y * scale_up),
                            0.,
                        ),
                    rotation: Quat::from_rotation_z(angle + custom_rotation.unwrap_or(0.)),
                    scale: Vec3::splat(scale_up),
                    ..default()
                }))
                .insert(ActiveEvents::COLLISION_EVENTS)
                .insert(Name::new("Projectile"))
                .insert(ActiveCollisionTypes::all())
                .remove::<ItemStack>();

            if let Some(sprite_map) = &params.graphics.spritesheet_map {
                if let Some(obj_type) = params.get_component::<WorldObject, _>(obj.clone()) {
                    if let Some(sprite) = sprite_map.get(obj_type) {
                        spawned_entity_commands
                            .insert(params.graphics.texture_atlas.as_ref().unwrap().clone())
                            .insert(sprite.clone())
                            .remove::<PendingSpriteSheet>();
                    }
                }
            }
            if let Some(arc_data) = params.get_component::<ArcProjectileData, _>(obj.clone()) {
                spawned_entity_commands.with_children(|parent| {
                    let angle = arc_data.col_points[0];
                    parent.spawn((
                        TransformBundle::from_transform(Transform {
                            translation: (Vec3::new(
                                (angle.cos() * (arc_data.size.x) + angle.cos() * (arc_data.size.y))
                                    / 2.,
                                (angle.sin() * (arc_data.size.x) + angle.sin() * (arc_data.size.y))
                                    / 2.,
                                0.,
                            )),
                            rotation: Quat::from_rotation_z((arc_data.col_points[0]) - PI / 2.),
                            ..default()
                        }),
                        Sensor,
                        Collider::cuboid(
                            arc_data.col_size.x * scale_up,
                            arc_data.col_size.y * scale_up,
                        ),
                        ActiveEvents::COLLISION_EVENTS,
                        ActiveCollisionTypes::all(),
                    ));
                });
            }
            if let Some(proj) = params.get_component::<Projectile, _>(obj.clone()) {
                if proj == &Projectile::Electricity {
                    spawned_entity_commands
                        .insert(AsepriteAnimation::from(Electricity::tags::ELECTRICITY))
                        .insert(asset_server.load::<Aseprite, _>(Electricity::PATH))
                        .remove::<TextureAtlasSprite>()
                        .remove::<Handle<TextureAtlas>>()
                        .remove::<AnimationTimer>();
                } else if proj == &Projectile::EnergyBall {
                    spawned_entity_commands
                        .insert(AsepriteAnimation::from("Bullet"))
                        .insert(asset_server.load::<Aseprite, _>("textures/effects/EnergyBall.ase"))
                        .remove::<TextureAtlasSprite>()
                        .remove::<Handle<TextureAtlas>>()
                        .remove::<AnimationTimer>();
                } else if proj == &Projectile::Bomb {
                    spawned_entity_commands
                        .insert(AsepriteAnimation::from("Bomb"))
                        .insert(asset_server.load::<Aseprite, _>("textures/effects/Bomb.ase"))
                        .remove::<TextureAtlasSprite>()
                        .remove::<Handle<TextureAtlas>>()
                        .remove::<AnimationTimer>();
                }
            }

            return Some(spawned_entity);
        }
        None
    }

    fn spawn_from_proto<'a, T: Display + Clone + Into<&'a str>>(
        &mut self,
        mob: T,
        defs: &GameDefs,
        pos: Vec2,
    ) -> Option<Entity> {
        let p = <T as Into<&str>>::into(mob).to_owned();
        let Some(def) = defs.get(&p) else {
            error!("GameDefs missing entity def: {p}");
            return None;
        };
        Some(spawn_from_def(self, def, pos))
    }

    fn spawn_object_from_proto<'a, T: Display + Clone + Into<&'a str>>(
        &mut self,
        obj: T,
        pos: Vec2,
        proto_param: &ProtoParam,
        is_dirty: bool,
    ) -> Option<Entity> {
        let p = <T as Into<&str>>::into(obj.clone()).to_owned();
        let Some(def) = proto_param.defs.get(&p) else {
            error!("GameDefs missing entity def: {p}");
            return None;
        };
        let spawned_entity = spawn_from_def(self, def, pos);
        let Some(mut spawned_entity_commands) = self.get_entity(spawned_entity) else {
            return None;
        };
        let relative_tile_pos = world_pos_to_chunk_relative_tile_pos(pos);
        let should_center = proto_param
            .get_component::<SpriteSize, _>(obj.clone())
            .unwrap_or(&SpriteSize::Small)
            .is_medium();
        let pos = tile_pos_to_world_pos(relative_tile_pos, should_center).extend(0.);
        let mut final_transform = Transform::from_translation(pos);
        if let Some(anchor) = proto_param.get_component::<SpriteAnchor, _>(obj.clone()) {
            final_transform.translation = pos + anchor.0.extend(0.);
        }
        spawned_entity_commands
            .insert(TransformBundle::from_transform(final_transform));

        if let Some(_wall) = proto_param.get_component::<Wall, _>(obj.clone()) {
            let sprite_data = proto_param
                .get_component::<WallTextureData, _>(obj.clone())
                .unwrap();
            spawned_entity_commands
                .insert(SpriteSheetBundle {
                    texture_atlas: proto_param
                        .graphics
                        .wall_texture_atlas
                        .as_ref()
                        .unwrap()
                        .clone(),
                    sprite: TextureAtlasSprite {
                        index: (sprite_data.obj_bit_index + sprite_data.texture_offset * 32)
                            as usize,
                        ..default()
                    },
                    transform: final_transform,
                    ..default()
                })
                .remove::<PendingSpriteSheet>()
                .remove::<PendingSpriteTexture>();
            if is_dirty {
                spawned_entity_commands.insert(Dirty);
            }
        } else if let Some(texture_path) = def.sprite_texture.as_ref() {
            // Match old SpriteBundle: use native image size, except BossShrine which
            // was always forced to 128² by the spawn failsafe.
            let custom_size = proto_param
                .get_world_object(obj.clone())
                .filter(|o| **o == WorldObject::BossShrine)
                .map(|_| Vec2::new(128., 128.));
            spawned_entity_commands
                .insert(SpriteBundle {
                    texture: proto_param.asset_server.load::<Image, _>(texture_path.as_str()),
                    sprite: Sprite {
                        custom_size,
                        ..default()
                    },
                    transform: final_transform,
                    ..default()
                })
                .remove::<PendingSpriteSheet>()
                .remove::<PendingSpriteTexture>()
                .remove::<TextureAtlasSprite>()
                .remove::<Handle<TextureAtlas>>();
        } else if let Some(sprite_map) = &proto_param.graphics.spritesheet_map {
            if let Some(obj_type) = proto_param.get_component::<WorldObject, _>(obj.clone()) {
                if crate::item::shrine_visuals::uses_standalone_shrine_texture(obj_type)
                    || obj_type == &WorldObject::PinkFlower
                {
                    // Art applied by shrine_visuals / special systems.
                } else if let Some(sprite) = sprite_map.get(obj_type) {
                    spawned_entity_commands
                        .insert(SpriteSheetBundle {
                            texture_atlas: proto_param
                                .graphics
                                .texture_atlas
                                .as_ref()
                                .unwrap()
                                .clone(),
                            sprite: sprite.clone(),
                            transform: final_transform,
                            ..default()
                        })
                        .remove::<PendingSpriteSheet>()
                        .remove::<PendingSpriteTexture>();
                }
            }
        }

        spawned_entity_commands.insert(Visibility::Inherited);

        Some(spawned_entity)
    }
}
