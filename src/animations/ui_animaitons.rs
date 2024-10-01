// allows animating Entities by adding special components to them.
// Move entities from point A to B (with or without acceleration)

use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};

use crate::{
    assets::Graphics,
    attributes::{add_item_glows, ItemRarity},
    colors::WHITE,
    inventory::ItemStack,
    item::WorldObject,
    player::ModifyTimeFragmentsEvent,
    proto::proto_param::ProtoParam,
    ui::damage_numbers::spawn_text,
};

#[derive(Component)]
pub struct MoveUIAnimation {
    pub start: Vec3,
    pub end: Vec3,
    pub velocity: f32,
    pub acceleration: Option<f32>,
    pub fade_factor: Option<f32>,
    pub despawn_when_done: bool,
    pub item_stack: ItemStack,
    pub startup_delay: Timer,
}

pub fn handle_move_animations(
    time: Res<Time>,
    mut query: Query<(
        Entity,
        &mut Transform,
        &mut MoveUIAnimation,
        &mut TextureAtlasSprite,
        Option<&Children>,
    )>,
    mut commands: Commands,
    mut child_text_query: Query<&mut Text>,
    mut currency_event: EventWriter<ModifyTimeFragmentsEvent>,
) {
    for (e, mut transform, mut move_anim, mut sprite, child_option) in query.iter_mut() {
        if !move_anim.startup_delay.tick(time.delta()).finished() {
            continue;
        }

        let direction = move_anim.end - move_anim.start;
        let curr_distance = (transform.translation - move_anim.start).length();
        let distance = direction.length();
        let velocity = move_anim.velocity * time.delta_seconds();
        let acceleration = move_anim.acceleration.unwrap_or(0.0);
        let new_velocity = velocity + acceleration * time.delta_seconds();

        let delta = direction.normalize() * new_velocity;

        if curr_distance >= distance {
            transform.translation = move_anim.end;
        } else if curr_distance < distance {
            transform.translation += delta;
            move_anim.velocity = new_velocity;
        }
        if move_anim.fade_factor.is_none() && curr_distance >= distance {
            if move_anim.item_stack.obj_type == WorldObject::TimeFragment {
                currency_event.send(ModifyTimeFragmentsEvent {
                    delta: move_anim.item_stack.count as i32,
                });
            }
            if move_anim.despawn_when_done {
                commands.entity(e).despawn_recursive();
            }
        }

        if let Some(fade) = move_anim.fade_factor {
            let new_fade = sprite.color.a() - fade * time.delta_seconds();
            sprite.color.set_a(new_fade);
            if sprite.color.a() <= 0.4 && move_anim.despawn_when_done {
                commands.entity(e).despawn_recursive();
            }

            if let Some(child) = child_option {
                for child_e in child.iter() {
                    if let Ok(mut text) = child_text_query.get_mut(*child_e) {
                        let new_fade =
                            text.sections[0].style.color.a() - fade * time.delta_seconds();
                        text.sections[0].style.color.set_a(new_fade);
                    }
                }
            }
        }
    }
}

#[derive(Component)]
pub struct UIIconMover {
    pub start: Vec3,
    pub end: Vec3,
    pub icon: WorldObject,
    pub velocity: f32,
    pub acceleration: f32,
    pub fade_factor: Option<f32>,
    pub show_name: bool,
    pub stack: ItemStack,
    pub despawn_when_done: bool,
}

impl UIIconMover {
    pub fn new(
        start: Vec3,
        end: Vec3,
        icon: WorldObject,
        velocity: f32,
        acceleration: f32,
        fade_factor: Option<f32>,
        show_name: bool,
        stack: ItemStack,
        despawn_when_done: bool,
    ) -> Self {
        UIIconMover {
            start,
            end,
            icon,
            velocity,
            acceleration,
            fade_factor,
            show_name,
            stack,
            despawn_when_done,
        }
    }
}
pub fn handle_ui_time_fragments(
    mut query: Query<(Entity, &mut UIIconMover), Added<UIIconMover>>,
    mut prev_movers: Query<&mut MoveUIAnimation>,
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    proto: ProtoParam,
) {
    let mut non_text_movers_this_frame = 0.;
    for (i, (e, icon)) in query.iter_mut().enumerate() {
        if icon.show_name {
            for mut mover in prev_movers.iter_mut() {
                mover.end.y += 11.5;
            }
        } else {
            non_text_movers_this_frame += 1.;
        }
        let icon_e = commands
            .entity(e)
            .insert(SpriteSheetBundle {
                sprite: graphics
                    .spritesheet_map
                    .as_ref()
                    .unwrap()
                    .get(&icon.icon)
                    .unwrap()
                    .clone(),
                texture_atlas: graphics.texture_atlas.as_ref().unwrap().clone(),

                transform: Transform::from_translation(icon.start),
                ..Default::default()
            })
            .insert(MoveUIAnimation {
                start: icon.start,
                end: icon.end + Vec3::new(0.0, (i as f32 - non_text_movers_this_frame) * 10.0, 0.),
                velocity: icon.velocity,
                acceleration: Some(icon.acceleration),
                fade_factor: icon.fade_factor,
                item_stack: icon.stack.clone(),
                despawn_when_done: icon.despawn_when_done,
                startup_delay: Timer::from_seconds(0.0, TimerMode::Once),
            })
            .insert(RenderLayers::from_layers(&[3]))
            .id();
        let obj_data = proto.get_item_data(icon.icon).unwrap();
        let obj_rarity = icon.stack.rarity.clone();
        let obj_name = obj_data.metadata.name.clone();

        if let Some(glow_e) = add_item_glows(&mut commands, &graphics, icon_e, obj_rarity.clone()) {
            commands
                .entity(glow_e)
                .insert(RenderLayers::from_layers(&[3]));
        }
        if icon.show_name {
            let text = spawn_text(
                &mut commands,
                &asset_server,
                Vec3::new(8., 0., 1.),
                if obj_rarity == ItemRarity::Common {
                    WHITE
                } else {
                    obj_rarity.get_color()
                },
                format!(
                    "{}{}",
                    obj_name,
                    if icon.stack.count > 1 {
                        format!(" x{}", icon.stack.count)
                    } else {
                        "".to_string()
                    }
                ),
                Anchor::CenterLeft,
                1.,
                3,
            );
            commands.entity(text).set_parent(icon_e);
        }
    }
}
