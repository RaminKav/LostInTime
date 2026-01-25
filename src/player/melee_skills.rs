use bevy::prelude::*;
use bevy_aseprite::{anim::AsepriteAnimation, aseprite, Aseprite};
use bevy_rapier2d::prelude::{Collider, KinematicCharacterController};

use crate::{
    animations::player_sprite::PlayerAnimation,
    attributes::{
        modifiers::ModifyHealthEvent, Attack, CurrentHealth, CurrentMana, HealthRegen,
        ProjectileSize,
    },
    audio::{AudioSoundEffect, SoundSpawner},
    colors::LIGHT_RED,
    combat_helpers::{spawn_deferred_aseprite_collider, spawn_temp_collider},
    cursor::CursorPos,
    enemy::Mob,
    inputs::MovementVector,
    item::{projectile::Projectile, WorldObject},
    status_effects::Frail,
    ui::damage_numbers::{spawn_floating_text_with_shadow, PreviousHealth},
    world::TILE_SIZE,
    GameParam, HitEvent,
};

use super::{ActiveSkill, Heirloom, Player, PlayerSkills};
aseprite!(pub Echo, "textures/effects/OnHitAoE.aseprite");

#[derive(Component)]
pub struct SecondHitDelay {
    pub delay: Timer,
    pub dir: Vec2,
    pub weapon_obj: WorldObject,
}

// OnHitEcho is now triggered in handle_modify_health_event (src/attributes/modifiers.rs)
// so that all sources of HP loss trigger the echo, not just combat hits.

pub fn handle_second_split_attack(
    mobs: Query<Option<&Frail>, With<Mob>>,
    game: GameParam,
    mut second_hit_query: Query<(Entity, &mut SecondHitDelay)>,
    mut hit_event: EventWriter<HitEvent>,
    time: Res<Time>,
    mut commands: Commands,
) {
    for (e, mut second_hit) in second_hit_query.iter_mut() {
        if !second_hit.delay.tick(time.delta()).just_finished() {
            continue;
        }
        let Ok(frail_option) = mobs.get(e) else {
            continue;
        };

        let frail_stacks = frail_option.map(|f| f.num_stacks).unwrap_or(0);
        let (damage, was_crit, was_overcrit) = game.calculate_player_damage(
            &mut commands,
            e,
            (frail_stacks * 5) as u32,
            None,
            0,
            None,
            frail_stacks,
        );

        let split_damage = f32::floor(damage as f32 / 2.) as i32;

        hit_event.send(HitEvent {
            hit_by_pet: None,
            hit_entity: e,
            damage: split_damage,
            dir: second_hit.dir,
            hit_with_melee: Some(second_hit.weapon_obj),
            hit_with_projectile: None,
            was_crit,
            was_overcrit,
            hit_by_mob: None,
            ignore_tool: false,
            from_heirloom_effect: false, // Split attack is a player skill, should trigger heirlooms
        });
        commands.entity(e).remove::<SecondHitDelay>();
    }
}

pub fn handle_echo_after_heal(
    mut commands: Commands,
    mut changed_health: Query<
        (
            Entity,
            &CurrentHealth,
            &PreviousHealth,
            &PlayerSkills,
            &Attack,
            &ProjectileSize,
            &mut CurrentMana,
        ),
        Changed<CurrentHealth>,
    >,
    asset_server: Res<AssetServer>,
) {
    for (e, changed_health, prev_health, skills, attack, projectile_size, mut current_mana) in
        changed_health.iter_mut()
    {
        let delta = changed_health.0 - prev_health.0;
        if delta <= 0 {
            continue;
        }

        if skills.has(Heirloom::HealEcho) {
            let mana_cost = Heirloom::HealEcho.get_mana_cost();
            if current_mana.0 >= mana_cost {
                current_mana.0 -= mana_cost;
                spawn_echo_hitbox(
                    &mut commands,
                    &asset_server,
                    e,
                    attack.0,
                    projectile_size.get_multiplier(),
                );
            }
        }
    }
}
#[derive(Component)]

pub struct SpearGravity {
    pub target: Vec2,
    pub timer: Timer,
}
#[derive(Component)]
pub struct ParryState {
    pub parry_timer: Timer,
    pub cooldown_timer: Timer,
    pub success: bool,
    pub active: bool,
}
#[derive(Component)]
pub struct SpearState {
    pub cooldown_timer: Timer,
    pub spear_timer: Timer,
}

#[derive(Component)]
pub struct SpearPullDelay {
    pub delay_timer: Timer,
    pub epicenter: Vec2,
}

#[derive(Component)]

pub struct Parried {
    pub timer: Timer,
    pub kb_applied: bool,
}

#[derive(Debug)]
pub struct ParrySuccessEvent(pub Entity);

#[derive(Component)]
pub struct SpearAttack;

pub fn handle_parry(
    mut player: Query<(Entity, &PlayerSkills, &PlayerAnimation, &mut ParryState), (With<Player>,)>,
    key_input: Res<Input<KeyCode>>,
    mouse_input: Res<Input<MouseButton>>,
    mut commands: Commands,
    time: Res<Time>,
    keybinds: Res<crate::keybinds::InputMappings>,
) {
    let Ok((e, skills, curr_anim, mut parry_state)) = player.get_single_mut() else {
        return;
    };

    if let Some(parry_slot) = skills.has_active_skill(ActiveSkill::Parry) {
        if keybinds.check_skill_input(parry_slot, &key_input, &mouse_input)
            && parry_state.cooldown_timer.finished()
            && !curr_anim.is_parrying()
        {
            commands.entity(e).insert(PlayerAnimation::Parry);
            parry_state.cooldown_timer.reset();
            parry_state.parry_timer.reset();
            parry_state.active = true;
            parry_state.parry_timer.tick(time.delta());
            // ActiveSkillUsedEvent dispatched centrally
        }
    }
    parry_state.cooldown_timer.tick(time.delta());

    if parry_state.parry_timer.percent() != 0. {
        parry_state.parry_timer.tick(time.delta());
        if parry_state.parry_timer.just_finished() {
            parry_state.success = false;
            parry_state.active = false;
        }
    }
}
pub fn handle_spear(
    mut player: Query<
        (
            Entity,
            &GlobalTransform,
            &PlayerSkills,
            &Attack,
            &mut SpearState,
            &mut KinematicCharacterController,
            &mut MovementVector,
        ),
        (With<Player>,),
    >,
    key_input: Res<Input<KeyCode>>,
    mouse_input: Res<Input<MouseButton>>,
    mut commands: Commands,
    time: Res<Time>,
    cursor_pos: Res<CursorPos>,
    keybinds: Res<crate::keybinds::InputMappings>,
) {
    let Ok((e, player_pos, skills, dmg, mut spear_state, mut kcc, mut mv)) =
        player.get_single_mut()
    else {
        return;
    };

    if let Some(spear_slot) = skills.has_active_skill(ActiveSkill::ParrySpear) {
        if keybinds.check_skill_input(spear_slot, &key_input, &mouse_input)
            && spear_state.cooldown_timer.finished()
        {
            spear_state.cooldown_timer.reset();
            spear_state.spear_timer.tick(time.delta());
            commands.entity(e).insert(PlayerAnimation::Spear);
            commands.spawn(SoundSpawner::new(AudioSoundEffect::Spear, 0.5));
            commands.spawn(SoundSpawner::new(AudioSoundEffect::SpearPull, 0.5).with_delay(0.32));

            // Calculate epicenter position in front of player
            let player_pos_2d = player_pos.translation().truncate();
            let direction =
                (cursor_pos.world_coords.truncate() - player_pos_2d).normalize_or_zero();
            let epicenter = player_pos_2d + direction * 1.7 * TILE_SIZE.x;

            // Add delay component to time with animation (brief delay before pull)
            commands.entity(e).insert(SpearPullDelay {
                delay_timer: Timer::from_seconds(0.45, TimerMode::Once), // Delay to sync with animation
                epicenter,
            });

            // ActiveSkillUsedEvent dispatched centrally
        }
    }
    if spear_state.spear_timer.percent() != 0. {
        spear_state.spear_timer.tick(time.delta());
        if spear_state.spear_timer.just_finished() {
            spear_state.spear_timer.reset();
        }
        // Prevent player movement during spear throw
        mv.0 = mv.0 * 0.;
        kcc.translation = Some(Vec2::new(mv.0.x, mv.0.y));
    }
    spear_state.cooldown_timer.tick(time.delta());
}

pub fn handle_parry_success(
    player: Query<
        (
            Entity,
            &Attack,
            &HealthRegen,
            &GlobalTransform,
            &PlayerSkills,
            &ProjectileSize,
        ),
        With<Player>,
    >,
    mut parry_success_event: EventReader<ParrySuccessEvent>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut modify_health_event: EventWriter<ModifyHealthEvent>,
) {
    for _ in parry_success_event.iter() {
        let (player_e, attack, health_regen, player_txfm, skills, projectile_size) =
            player.single();
        spawn_floating_text_with_shadow(
            &mut commands,
            &asset_server,
            player_txfm.translation() + Vec3::new(0., 16., 0.),
            LIGHT_RED,
            "Parry!".to_string(),
        );
        if skills.has(Heirloom::ParryHPRegen) {
            modify_health_event.send(ModifyHealthEvent(health_regen.0));
        }
        if skills.has(Heirloom::ParryEcho) {
            spawn_echo_hitbox(
                &mut commands,
                &asset_server,
                player_e,
                attack.0,
                projectile_size.get_multiplier(),
            );
        }
    }
}

pub fn tick_parried_timer(
    mut parried_query: Query<(Entity, &mut Parried)>,
    time: Res<Time>,
    mut commands: Commands,
) {
    for (e, mut parried) in parried_query.iter_mut() {
        parried.timer.tick(time.delta());
        if parried.timer.just_finished() {
            commands.entity(e).remove::<Parried>();
        }
    }
}

pub fn spawn_echo_hitbox(
    commands: &mut Commands,
    asset_server: &AssetServer,
    player: Entity,
    dmg: i32,
    size_multiplier: f32,
) {
    // Use default animation to ensure it starts at frame 0
    let anim = AsepriteAnimation::default();

    // Scale the collider radius by the size multiplier
    let base_radius = 24.0;
    let scaled_radius = base_radius * size_multiplier;

    // Queue deferred spawn with parent - actual entity will be created in PreUpdate and parented
    spawn_deferred_aseprite_collider(
        commands,
        Transform::from_translation(Vec3::ZERO).with_scale(Vec3::splat(size_multiplier)),
        10.5,
        dmg,
        Collider::capsule(Vec2::ZERO, Vec2::ZERO, scaled_radius),
        asset_server.load::<Aseprite, _>(Echo::PATH),
        anim,
        false,
        Projectile::Echo,
        vec![],       // No extra components needed
        Some(player), // Parent to player entity
    );
}

pub fn handle_spear_pull_delay(
    mut player_query: Query<(Entity, &mut SpearPullDelay, &Attack), With<Player>>,
    mobs: Query<(Entity, &GlobalTransform), With<Mob>>,
    time: Res<Time>,
    mut commands: Commands,
) {
    for (player_e, mut pull_delay, attack) in player_query.iter_mut() {
        pull_delay.delay_timer.tick(time.delta());
        if !pull_delay.delay_timer.just_finished() {
            continue;
        }

        // Pull all nearby enemies to the epicenter
        let epicenter = pull_delay.epicenter;
        let pull_radius = 128.0; // Radius to find enemies to pull

        for (mob_e, mob_transform) in mobs.iter() {
            let mob_pos = mob_transform.translation().truncate();
            let distance = (mob_pos - epicenter).length();

            if distance <= pull_radius {
                commands.entity(mob_e).insert(SpearGravity {
                    target: epicenter,
                    timer: Timer::from_seconds(0.5, TimerMode::Once),
                });
            }
        }

        // Spawn 20px damage hitbox at epicenter
        let hitbox = spawn_temp_collider(
            &mut commands,
            Transform::from_translation(Vec3::new(epicenter.x, epicenter.y, 1.0)),
            0.5, // Very short duration, just for the hit
            attack.0,
            Collider::ball(18.0), // 20px radius
            Projectile::Echo,
        );
        commands.entity(hitbox).insert(SpearAttack);

        // Remove the delay component
        commands.entity(player_e).remove::<SpearPullDelay>();
    }
}

pub fn handle_spear_gravity(
    mut spear_query: Query<(
        Entity,
        &mut SpearGravity,
        &mut GlobalTransform,
        &mut KinematicCharacterController,
    )>,
    time: Res<Time>,
    mut commands: Commands,
) {
    for (e, mut spear_gravity, txfm, mut kcc) in spear_query.iter_mut() {
        spear_gravity.timer.tick(time.delta());
        if spear_gravity.timer.just_finished() {
            commands.entity(e).remove::<SpearGravity>();
            continue;
        }
        let distance = spear_gravity.target - txfm.translation().truncate();
        let delta = (distance).normalize_or_zero();
        if distance.length() < 16. {
            commands.entity(e).remove::<SpearGravity>();
            continue;
        }

        kcc.translation = Some(delta * 300. * time.delta_seconds());
    }
}
