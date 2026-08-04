use bevy::text::Justify;
use crate::aseprite_helpers::pause;
use crate::ui::game_fonts as gf;
use bevy_aseprite_ultra::prelude::AseAnimation;
use std::{fs::File, io::BufReader};

use bevy::color::Alpha;
use bevy::{camera::visibility::RenderLayers, prelude::*, sprite::Anchor};
use rand::seq::IteratorRandom;

use crate::{
    assets::Graphics,
    attributes::{
        Attack, AttackSpeed, AttributeQuality, AttributeValue, BonusDamage, CritChance, CritDamage,
        CurrentHealth, CurrentMana, Defence, Dodge, Healing, ItemAttributes, Lifesteal,
        LootRateBonus, ManaRegen, MaxHealth, MaxMana, PickupRange, ProjectileSize, SkillPower,
        Speed, Thorns, XpRateBonus,
    },
    chaos::ChaosTracker,
    client::{leaderboard::LastSubmittedScore, GameData, GameOverEvent},
    colors::{overwrite_alpha, WHITE, YELLOW_2},
    combat::damage_tracker::{
        spawn_damage_tracker_ui, spawn_mob_stat_tracker_ui, DamageTracker, MobStatTracker,
        PetAbilityStats,
    },
    datafiles,
    inputs::FacingDirection,
    inventory::ItemStack,
    item::WorldObject,
    night::InfiniteMode,
    player::{
        levels::PlayerLevel,
        score::{RunScore, RunTimer},
        skills::PlayerSkills,
        Player, TimeFragmentCurrency,
    },
    proto::proto_param::ProtoParam,
    ui::{
        boss_health_bar::{BossHealthBar, BossHealthBarFrame, BossNameText},
        damage_numbers::spawn_text,
        game_fonts::FLOATING_TEXT,
        key_input_guide::InteractGuide,
        player_hud::{HudGameOverHeirloomSlide, SkillHudIcon},
        spawn_item_stack_icon, spawn_stats_tooltip_at,
        ui_helpers::{self, Z_DEPTH_HEIRLOOM_SKILL_CHOICE_FOREGROUND},
        CurrencyText, Interactable, Interaction, MenuButton, TimeFragmentIcon, UIElement, UIState,
        HUD_GAME_OVER_LEFT_PANEL_Y_OFFSET, HUD_HEIRLOOM_GAME_OVER_SLIDE_SECS,
        HUD_HEIRLOOM_GAME_OVER_Y_OFFSET,
    },
    world::{
        dimension::{Era, EraManager},
        y_sort::YSort,
    },
    GameState, RawPosition, ScreenResolution,
};

use super::ui_animaitons::{MoveUIAnimation, UIIconMover};

#[derive(Component)]
pub struct GameOverFadeout(Timer);

impl GameOverFadeout {
    pub fn progress(&self) -> f32 {
        self.0.fraction()
    }
}

/// Era reached as display number (1, 2, or 3; dungeon counts as its associated era).
fn era_display_number(era: &Era) -> u8 {
    match era {
        Era::Main | Era::DungeonMain => 1,
        Era::Second => 2,
        Era::Third => 3,
    }
}

/// Helper function to format rank with ordinal suffix (1st, 2nd, 3rd, 4th, etc.)
fn format_rank(rank: i64) -> String {
    let suffix = match rank % 10 {
        1 if rank % 100 != 11 => "st",
        2 if rank % 100 != 12 => "nd",
        3 if rank % 100 != 13 => "rd",
        _ => "th",
    };
    format!("#{}{} Worldwide", rank, suffix)
}

fn format_run_time_mm_ss(elapsed_seconds: f64) -> String {
    let s = elapsed_seconds.max(0.0);
    let minutes = (s / 60.0) as u32;
    let secs = (s % 60.0).floor() as u32;
    format!("{:02}:{:02}", minutes, secs)
}

pub fn handle_game_over_fadeout(
    mut commands: Commands,
    game_over_events: MessageReader<GameOverEvent>,
    mut next_state: ResMut<NextState<GameState>>,
    mut player: Query<
        (
            Entity,
            &FacingDirection,
            &mut Transform,
            &mut AseAnimation,
            &mut Sprite,
            &PlayerLevel,
        ),
        With<Player>,
    >,
    asset_server: Res<AssetServer>,
    graphics: Res<Graphics>,
    mut next_ui_state: ResMut<NextState<UIState>>,
    resolution: Res<ScreenResolution>,
    run_score: Res<RunScore>,
    run_timer: Res<RunTimer>,
    last_submitted: Res<LastSubmittedScore>,
    mut trackers: ParamSet<(
        Res<DamageTracker>,
        Res<EraManager>,
        Res<ChaosTracker>,
        Option<Res<InfiniteMode>>,
    )>,
    mob_stat_tracker: Res<MobStatTracker>,
    pet_stats: Res<PetAbilityStats>,
    // Cleanup queries
    boss_health_bars: Query<
        Entity,
        Or<(
            With<BossHealthBar>,
            With<BossHealthBarFrame>,
            With<BossNameText>,
        )>,
    >,
    guide_hud: Query<Entity, With<InteractGuide>>,
) {
    if !game_over_events.is_empty() {
        // Clean up boss health bars and guide HUD
        for entity in boss_health_bars.iter() {
            commands.entity(entity).despawn();
        }
        for entity in guide_hud.iter() {
            commands.entity(entity).despawn();
        }
        let Ok((player_e, dir, mut player_t, mut anim, mut sprite, player_level)) =
            player.single_mut()
        else {
            return;
        };
        next_ui_state.set(UIState::Closed);
        // BLACK OVERLAY
        commands
            .spawn((
                Sprite {
                    color: Color::srgba(0., 0., 0., 0.),
                    custom_size: Some(crate::ui::ui_helpers::full_screen_overlay_size(&resolution)),
                    ..default()
                },
                Transform {
                    translation: Vec3::new(0., 0., Z_DEPTH_HEIRLOOM_SKILL_CHOICE_FOREGROUND - 1.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
            ))
            .insert(RenderLayers::from_layers(&[3]))
            .insert(Name::new("overlay"))
            .insert(GameOverFadeout(Timer::from_seconds(6.5, TimerMode::Once)));
        // GAME OVER TEXT
        commands.spawn((
            gf::DISPLAY_LARGE
                .text(&asset_server, "Game Over", WHITE.with_alpha(0.))
                .with_transform(Transform {
                    translation: Vec3::new(0., 100., Z_DEPTH_HEIRLOOM_SKILL_CHOICE_FOREGROUND),
                    scale: gf::DISPLAY_LARGE.transform_scale(),
                    ..Default::default()
                }),
            GameOverText,
            RenderLayers::from_layers(&[3]),
        ));

        // RANK TEXT - spawn as "Submitting..." initially, will be updated by a system
        let rank_text = if let Some(rank) = last_submitted.rank {
            format_rank(rank)
        } else {
            "Submitting to leaderboard...".to_string()
        };

        commands.spawn((
            gf::DISPLAY
                .text(&asset_server, rank_text, WHITE.with_alpha(0.))
                .with_transform(Transform {
                    translation: Vec3::new(0., 64., Z_DEPTH_HEIRLOOM_SKILL_CHOICE_FOREGROUND), // Moved 30px higher
                    scale: gf::DISPLAY.transform_scale(),
                    ..Default::default()
                }),
            GameOverText,
            GameOverRankText, // Special marker for updating
            RenderLayers::from_layers(&[3]),
            Name::new("Rank Text2d"),
        ));

        // Left panel position (same as damage tracker)
        let panel_x = -resolution.game_width / 2. + 10.;

        // SCORE TEXT - same style and alignment as damage display
        commands.spawn((
            gf::DISPLAY
                .text(
                    &asset_server,
                    format!("Score: {}", run_score.score),
                    WHITE.with_alpha(0.),
                )
                .with_transform(Transform {
                    translation: Vec3::new(0., 48., Z_DEPTH_HEIRLOOM_SKILL_CHOICE_FOREGROUND), // Moved 30px higher
                    scale: gf::DISPLAY.transform_scale(),
                    ..Default::default()
                }),
            GameOverText,
            RenderLayers::from_layers(&[3]),
            Name::new("Score Text2d"),
        ));
        // DAMAGE BREAKDOWN + mob stats — left side list with category headers
        let start_y = resolution.game_height / 2. - 60. + HUD_GAME_OVER_LEFT_PANEL_Y_OFFSET;
        let stats_width = 84.0;
        let mut next_y = start_y;

        let damage_tracker = trackers.p0();

        if let Some((entities, dmg_bottom_y)) = spawn_damage_tracker_ui(
            &mut commands,
            &asset_server,
            &damage_tracker,
            Transform::from_translation(Vec3::new(
                panel_x + 42.0,
                next_y,
                Z_DEPTH_HEIRLOOM_SKILL_CHOICE_FOREGROUND,
            )),
            0.0,
            stats_width,
            Some(&pet_stats),
        ) {
            for e in entities {
                commands.entity(e).insert(GameOverText);
            }
            next_y += dmg_bottom_y - 10.0;
        }

        if let Some((entities, _)) = spawn_mob_stat_tracker_ui(
            &mut commands,
            &asset_server,
            &mob_stat_tracker,
            Transform::from_translation(Vec3::new(
                panel_x + 42.0,
                next_y,
                Z_DEPTH_HEIRLOOM_SKILL_CHOICE_FOREGROUND,
            )),
            0.0,
            stats_width,
        ) {
            for e in entities {
                commands.entity(e).insert(GameOverText);
            }
        }

        // Run summary on the right, below time crystal count (same x anchor as `CurrencyText` in `tick_game_over_overlay`)
        let right_stats_x = resolution.game_width / 2. - 100.;
        let line_step = 10.0;
        let mut line_y = resolution.game_height / 2. - 76.;

        let era_manager = trackers.p1();
        let era_num = era_display_number(&era_manager.current_era);
        commands.spawn((
            gf::BODY
                .text(
                    &asset_server,
                    format!("Era Reached: {}", era_num),
                    WHITE.with_alpha(0.),
                )
                .justify(Justify::Left)
                .anchor(Anchor::CENTER_LEFT)
                .with_transform(Transform {
                    translation: Vec3::new(
                        right_stats_x,
                        line_y,
                        Z_DEPTH_HEIRLOOM_SKILL_CHOICE_FOREGROUND,
                    ),
                    scale: gf::BODY.transform_scale(),
                    ..Default::default()
                }),
            GameOverText,
            RenderLayers::from_layers(&[3]),
            Name::new("Era Text2d"),
        ));
        line_y -= line_step;

        let chaos_tracker = trackers.p2();
        commands.spawn((
            gf::BODY
                .text(
                    &asset_server,
                    format!("Chaos Level: {:.1}", chaos_tracker.get_chaos()),
                    WHITE.with_alpha(0.),
                )
                .justify(Justify::Left)
                .anchor(Anchor::CENTER_LEFT)
                .with_transform(Transform {
                    translation: Vec3::new(
                        right_stats_x,
                        line_y,
                        Z_DEPTH_HEIRLOOM_SKILL_CHOICE_FOREGROUND,
                    ),
                    scale: gf::BODY.transform_scale(),
                    ..Default::default()
                }),
            GameOverText,
            RenderLayers::from_layers(&[3]),
            Name::new("Chaos Text2d"),
        ));
        line_y -= line_step;

        commands.spawn((
            gf::BODY
                .text(
                    &asset_server,
                    format!(
                        "Run Time: {}",
                        format_run_time_mm_ss(run_timer.elapsed_seconds)
                    ),
                    WHITE.with_alpha(0.),
                )
                .justify(Justify::Left)
                .anchor(Anchor::CENTER_LEFT)
                .with_transform(Transform {
                    translation: Vec3::new(
                        right_stats_x,
                        line_y,
                        Z_DEPTH_HEIRLOOM_SKILL_CHOICE_FOREGROUND,
                    ),
                    scale: gf::BODY.transform_scale(),
                    ..Default::default()
                }),
            GameOverText,
            RenderLayers::from_layers(&[3]),
            Name::new("Run Time Text2d"),
        ));
        line_y -= line_step;

        let infinite_mode = trackers.p3();
        if let Some(ref inf) = infinite_mode {
            if inf.active && inf.elapsed_seconds > 0.0 {
                commands.spawn((
                    gf::BODY
                        .text(
                            &asset_server,
                            format!("Time in Endless: {}", inf.get_elapsed_display_string()),
                            WHITE.with_alpha(0.),
                        )
                        .justify(Justify::Left)
                        .anchor(Anchor::CENTER_LEFT)
                        .with_transform(Transform {
                            translation: Vec3::new(
                                right_stats_x,
                                line_y,
                                Z_DEPTH_HEIRLOOM_SKILL_CHOICE_FOREGROUND,
                            ),
                            scale: gf::BODY.transform_scale(),
                            ..Default::default()
                        }),
                    GameOverText,
                    RenderLayers::from_layers(&[3]),
                    Name::new("Endless Time Text2d"),
                ));
                line_y -= line_step;
            }
        }

        commands.spawn((
            gf::BODY
                .text(
                    &asset_server,
                    format!("Level: {}", player_level.level),
                    WHITE.with_alpha(0.),
                )
                .justify(Justify::Left)
                .anchor(Anchor::CENTER_LEFT)
                .with_transform(Transform {
                    translation: Vec3::new(
                        right_stats_x,
                        line_y,
                        Z_DEPTH_HEIRLOOM_SKILL_CHOICE_FOREGROUND,
                    ),
                    scale: gf::BODY.transform_scale(),
                    ..Default::default()
                }),
            GameOverText,
            RenderLayers::from_layers(&[3]),
            Name::new("Game Over Level Text2d"),
        ));

        // FINAL STATS — centered, just above "Try Again"
        const TRY_AGAIN_BUTTON_Y: f32 = -99.;
        let view_stats_y = TRY_AGAIN_BUTTON_Y + 9. + 8. + 7.;
        let final_stats_hit = commands
            .spawn((
                (
                    Sprite {
                        color: Color::srgba(0.35, 0.35, 0.35, 0.),
                        custom_size: Some(Vec2::new(80., 14.)),
                        ..default()
                    },
                    Transform::from_translation(Vec3::new(
                        0.,
                        view_stats_y,
                        Z_DEPTH_HEIRLOOM_SKILL_CHOICE_FOREGROUND + 1.,
                    )),
                ),
                Interactable::default(),
                GameOverFinalStatsHitbox,
                crate::ui::focus::OverlayFocusable { index: 1 },
                GameOverText,
                RenderLayers::from_layers(&[3]),
                Name::new("Game Over Final Stats Hitbox"),
            ))
            .id();
        commands
            .spawn((
                gf::BODY
                    .text(&asset_server, "View Final Stats", YELLOW_2.with_alpha(0.))
                    .justify(Justify::Center)
                    .anchor(Anchor::CENTER)
                    .with_transform(Transform {
                        translation: Vec3::new(0., 0., 1.),
                        scale: gf::BODY.transform_scale(),
                        ..Default::default()
                    }),
                GameOverText,
                RenderLayers::from_layers(&[3]),
                Name::new("Game Over Final Stats Text2d"),
            ))
            .insert(ChildOf(final_stats_hit));

        // OK BUTTON - spawn like main menu buttons
        let button_entity = commands
            .spawn((
                (
                    Sprite {
                        image: graphics.get_ui_element_texture(UIElement::UnlocksButton),
                        color: Color::srgba(1.0, 1.0, 1.0, 0.0), // Start transparent
                        custom_size: Some(Vec2::new(84., 18.)),
                        ..default()
                    },
                    Transform::from_translation(Vec3::new(
                        0.,
                        -99.,
                        Z_DEPTH_HEIRLOOM_SKILL_CHOICE_FOREGROUND + 2.,
                    )),
                ),
                Interactable::default(),
                UIElement::UnlocksButton,
                MenuButton::GameOverOK,
                crate::ui::focus::OverlayFocusable { index: 0 },
                GameOverText,
                RenderLayers::from_layers(&[3]),
                Name::new("Game Over OK Button"),
            ))
            .id();

        // Button text as child
        commands
            .spawn((
                gf::DISPLAY
                    .text(&asset_server, "Try Again", WHITE.with_alpha(0.))
                    .with_transform(Transform {
                        translation: Vec3::new(0., 0., 1.),
                        scale: gf::DISPLAY.transform_scale(),
                        ..Default::default()
                    }),
                GameOverText,
                RenderLayers::from_layers(&[3]),
                Name::new("Game Over OK Text2d"),
            ))
            .insert(ChildOf(button_entity));
        next_state.set(GameState::GameOver);
        // move player to UI camera to be above the fade out overlay
        commands
            .entity(player_e)
            .remove::<YSort>()
            .remove::<RawPosition>()
            .insert(RenderLayers::from_layers(&[3]));
        player_t.translation = Vec3::new(0., 0., 100.);

        // Tint player sprite red instead of swapping to death sprite
        pause(&mut anim);
        sprite.color = Color::srgba(1.0, 0.2, 0.2, 1.0); // Brighter red tint, fully opaque
        if dir == &FacingDirection::Left {
            sprite.flip_x = true;
        }

        // Add a marker component to ensure color persists
        commands.entity(player_e).insert(GameOverPlayerTint);
    }
}

/// Marker component for the game over player tint
#[derive(Component)]
pub struct GameOverPlayerTint;

/// System to ensure player stays red during game over
pub fn maintain_player_red_tint(
    mut player: Query<&mut Sprite, (With<Player>, With<GameOverPlayerTint>)>,
) {
    if let Ok(mut sprite) = player.single_mut() {
        // Keep the sprite red
        if sprite.color != Color::srgba(1.0, 0.2, 0.2, 1.0) {
            sprite.color = Color::srgba(1.0, 0.2, 0.2, 1.0);
        }
    }
}
/// Hitbox for "Final Stats" on game over; hover shows player stats tooltip.
#[derive(Component)]
pub struct GameOverFinalStatsHitbox;

/// Marks the stats tooltip spawned on game over (hover "Final Stats") so we can despawn it when hover ends.
#[derive(Component)]
pub struct GameOverStatsTooltip;

#[derive(Component)]
pub struct GameOverText;

#[derive(Component)]
pub struct GameOverRankText;

#[derive(Resource)]
pub struct GameOverUITracker {
    pub game_over_text_check: bool,
    pub tip_check: bool,
    pub analytics_check: bool,
}
pub fn tick_game_over_overlay(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut GameOverFadeout, &mut Sprite), Without<GameOverText>>,
    asset_server: Res<AssetServer>,
    mut game_over_text: Query<&mut TextColor, With<GameOverText>>,
    mut game_over_sprites: Query<&mut Sprite, (With<GameOverText>, Without<GameOverFadeout>)>,
    mut heirloom_icons: Query<(&mut Transform, &mut HudGameOverHeirloomSlide), With<SkillHudIcon>>,
    mut heirloom_untagged: Query<
        (Entity, &Transform),
        (With<SkillHudIcon>, Without<HudGameOverHeirloomSlide>),
    >,
    mut tip_check: Local<bool>,
    graphics: Res<Graphics>,
    res: Res<ScreenResolution>,
    time_fragments: Res<TimeFragmentCurrency>,
) {
    let overlay_active = query.iter().next().is_some();
    if !overlay_active {
        *tip_check = false;
    }

    if overlay_active {
        for (entity, transform) in heirloom_untagged.iter() {
            commands.entity(entity).insert(HudGameOverHeirloomSlide {
                start_y: transform.translation.y,
                timer: Timer::from_seconds(HUD_HEIRLOOM_GAME_OVER_SLIDE_SECS, TimerMode::Once),
            });
        }
    }

    for (mut transform, mut slide) in heirloom_icons.iter_mut() {
        slide.timer.tick(time.delta());
        let t = slide.timer.fraction().clamp(0., 1.);
        transform.translation.y = slide.start_y + HUD_HEIRLOOM_GAME_OVER_Y_OFFSET * t;
    }

    for (_e, mut timer, mut sprite) in query.iter_mut() {
        if timer.0.fraction() >= 0.25 && !*tip_check {
            *tip_check = true;
            // Try to load tips from save
            let tips = vec![
              "Enemies get tougher every night. If you take too long, they will overpower you!",
              "Stars represent the overall quality of the stat lines on equipment.",
              // "Press Shift while inspecting an item to view the range\n\n     of possible values for each stat line.",
              "You can drop an item by dragging it out of your inventory.\n       Or put it in the trash slot in your inventory.",
              "Elite mobs are much tougher, but they give more exp and drop more loot.",
              "At night, enemies will spawn and move much faster. Be prepared!",
              "Item colors correspond to rarity:\n\n     Common (Grey), Uncommon (blue), Rare (purple), Legendary (yellow).",
              "Press Shift + Left Click to quickly move items\n\n     between your hotbar and inventory.",
              "Enemies drop higher level gear the higher level you are!\n\n      Higher level gear has better base stats.",
              // "Rolling through Crates instantly breaks them!",
              "Increasing Chaos is dangerous, but grants more score!",
              "Enemies drop mana orbs sometimes which recover 10 mana!",
              "Pink Flowers will bounce forward if you walk over them!",
              "Don't forget to upgrade your equipment in your inventory!\n        using Upgrade Tomes and Orbs.",
            ];

            let picked_tip = tips.iter().choose(&mut rand::thread_rng()).unwrap();
            commands
                .spawn(
                    gf::BODY
                        .text(
                            &asset_server,
                            format!("Tip: {}", picked_tip).to_string(),
                            WHITE,
                        )
                        .anchor(Anchor::CENTER)
                        .with_transform(Transform {
                            translation: Vec3::new(
                                0.,
                                -res.game_height / 2. + 56.5,
                                Z_DEPTH_HEIRLOOM_SKILL_CHOICE_FOREGROUND,
                            ),
                            scale: gf::BODY.transform_scale(),
                            ..Default::default()
                        }),
                )
                .insert(RenderLayers::from_layers(&[3]));

            // total currency counter
            let time_fragments = time_fragments.as_ref();
            let currency_this_run = time_fragments.total_collected_time_fragments_this_run as u32;
            let game_data_file_path = datafiles::game_data();
            let mut total_currency = time_fragments.time_fragments.max(0) as u128;
            if let Ok(file_file) = File::open(game_data_file_path) {
                let reader = BufReader::new(file_file);

                // Read the JSON contents of the file as an instance of `GameData`.
                match GameData::try_from_json_reader(reader) {
                    Ok(data) => total_currency = data.time_fragments,
                    Err(err) => error!(
                        "Failed to load data from game_data.json file to get currency {err:?}"
                    ),
                }
            };
            let currency_before_run = total_currency.saturating_sub(currency_this_run as u128);

            let text = spawn_text(
                &mut commands,
                &asset_server,
                Vec3::new(
                    res.game_width / 2. - 45.,
                    res.game_height / 2. - 43.5,
                    Z_DEPTH_HEIRLOOM_SKILL_CHOICE_FOREGROUND,
                ),
                WHITE,
                format!("{:}", currency_before_run),
                Anchor::CENTER_LEFT,
                FLOATING_TEXT,
                3,
                None,
            );
            commands.entity(text).insert(CurrencyText);

            let stack = spawn_item_stack_icon(
                &mut commands,
                &graphics,
                &ItemStack::crate_icon_stack(WorldObject::TimeFragment),
                &asset_server,
                Vec2::new(-8., 1.),
                Vec2::new(0., 0.),
                3,
            );
            commands
                .entity(stack)
                .insert(TimeFragmentIcon)
                .insert(ChildOf(text));

            commands.spawn(GameEndTimeFragmentSpawner {
                timer: Timer::from_seconds(0.05, TimerMode::Once),
                total_spawns: currency_this_run,
                remaining_spawns: currency_this_run,
            });
        }
        timer.0.tick(time.delta());

        let alpha = f32::min(1., timer.0.fraction() * 5.);
        sprite.color = overwrite_alpha(sprite.color, alpha);
        if alpha >= 0.45 {
            let text_alpha = f32::min(1., timer.0.fraction() * 2.);

            // Update text alpha
            game_over_text.iter_mut().for_each(|mut text_color| {
                text_color.0 = overwrite_alpha(text_color.0, text_alpha);
            });

            // Update button sprite alpha
            game_over_sprites.iter_mut().for_each(|mut s| {
                s.color = overwrite_alpha(s.color, text_alpha);
            });
        }
    }
}

#[derive(Component)]
pub struct GameEndTimeFragmentSpawner {
    pub timer: Timer,
    pub total_spawns: u32,
    pub remaining_spawns: u32,
}
pub fn handle_spawn_collected_time_fragments(
    mut commands: Commands,
    mut spawner_query: Query<(Entity, &mut GameEndTimeFragmentSpawner)>,
    time: Res<Time>,
    proto: ProtoParam,
    res: Res<ScreenResolution>,
    mut all_time_fragments: Query<(&GlobalTransform, &mut MoveUIAnimation)>,
) {
    for (e, mut spawner) in spawner_query.iter_mut() {
        spawner.timer.tick(time.delta());
        if spawner.timer.just_finished() {
            spawner.timer.reset();
            if spawner.remaining_spawns > 0 {
                let spacing = 5.;
                let total_offset =
                    f32::min(spawner.total_spawns as f32 * spacing, res.game_width * 0.8);
                let max_per_row = res.game_width * 0.8 / spacing;
                let i = spawner.total_spawns - spawner.remaining_spawns;
                let row_i = i % max_per_row as u32;
                let col_i = i / max_per_row as u32;
                let stack = proto
                    .get_item_data(WorldObject::TimeFragment)
                    .unwrap()
                    .clone()
                    .copy_with_count(0);
                commands.spawn(UIIconMover::new(
                    Vec3::new(0., 10., Z_DEPTH_HEIRLOOM_SKILL_CHOICE_FOREGROUND),
                    Vec3::new(
                        -total_offset / 2. + row_i as f32 * spacing,
                        -10. + (col_i as f32 + 1.) * -8.,
                        Z_DEPTH_HEIRLOOM_SKILL_CHOICE_FOREGROUND,
                    ),
                    WorldObject::TimeFragment,
                    0.,
                    800.,
                    None,
                    false,
                    stack,
                    false,
                ));
                spawner.remaining_spawns -= 1;
            } else {
                for (txfm, mut mover) in all_time_fragments.iter_mut() {
                    if mover.end == txfm.translation() {
                        mover.start = txfm.translation();
                        mover.end = Vec3::new(
                            res.game_width / 2. - 30.,
                            res.game_height / 2. - 43.5,
                            Z_DEPTH_HEIRLOOM_SKILL_CHOICE_FOREGROUND,
                        );
                        mover.startup_delay = Timer::from_seconds(1.0, TimerMode::Once);
                        mover.despawn_when_done = true;
                        mover.item_stack.count = 1;
                    }
                }
                if all_time_fragments.iter().count() == 0 {
                    commands.entity(e).despawn();
                }
            }
        }
    }
}

/// System to update the rank text on game over screen when rank becomes available
pub fn update_game_over_rank_text(
    last_submitted: Res<LastSubmittedScore>,
    mut rank_text_query: Query<&mut Text2d, With<GameOverRankText>>,
) {
    // Only update if the rank changed
    if !last_submitted.is_changed() {
        return;
    }

    if let Ok(mut text) = rank_text_query.single_mut() {
        if let Some(rank) = last_submitted.rank {
            // Update the text to show the actual rank
            text.0 = format_rank(rank);
        }
    }
}

/// Handle hover on "Final Stats" and show/hide the player stats tooltip.
pub fn handle_game_over_final_stats_tooltip(
    mut commands: Commands,
    cursor_pos: Res<crate::cursor::CursorPos>,
    ui_focus: Res<crate::ui::focus::UiFocus>,
    hit_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut hitbox_query: Query<(Entity, &mut Interactable), With<GameOverFinalStatsHitbox>>,
    existing_tooltips: Query<Entity, With<GameOverStatsTooltip>>,
    player_stats: Query<
        (
            (
                &Attack,
                &MaxHealth,
                &CurrentHealth,
                &MaxMana,
                &CurrentMana,
                &Defence,
                &CritChance,
                &CritDamage,
                &BonusDamage,
                &ManaRegen,
                &Healing,
                &Thorns,
                &Dodge,
                &Speed,
                &XpRateBonus,
            ),
            &LootRateBonus,
            &ProjectileSize,
            &SkillPower,
            &Lifesteal,
            &PickupRange,
            &AttackSpeed,
            &PlayerSkills,
        ),
        With<Player>,
    >,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
) {
    let hit_entity = ui_helpers::pointcast_2d(&cursor_pos, &hit_sprites, None, None);
    let mut panel_e = None;
    for (entity, mut interactable) in hitbox_query.iter_mut() {
        let is_hit = hit_entity
            .as_ref()
            .map(|(e, _sprite, _transform)| *e == entity)
            .unwrap_or(false)
            || ui_focus.is_focused(entity);

        if is_hit {
            panel_e = Some(entity);
            if !matches!(interactable.current(), Interaction::Hovering) {
                interactable.change(Interaction::Hovering);
            }
        } else if !is_hit && matches!(interactable.current(), Interaction::Hovering) {
            interactable.change(Interaction::None);
            panel_e = None;
        }
    }

    let is_hovering = hitbox_query
        .iter()
        .any(|(_, i)| matches!(i.current(), Interaction::Hovering));

    if is_hovering && panel_e.is_some() {
        if existing_tooltips.is_empty() {
            let Ok((
                (
                    attack,
                    max_health,
                    curr_health,
                    max_mana,
                    curr_mana,
                    defence,
                    crit_chance,
                    crit_damage,
                    bonus_damage,
                    mana_regen,
                    healing,
                    thorns,
                    dodge,
                    speed,
                    xp_rate_bonus,
                ),
                loot_rate_bonus,
                size,
                skill_power,
                lifesteal,
                pickup_range,
                attack_speed,
                skills,
            )) = player_stats.single()
            else {
                return;
            };

            let mut attributes = ItemAttributes {
                attack: AttributeValue::new(attack.0, AttributeQuality::Low, 0.),
                health: AttributeValue::new(max_health.0, AttributeQuality::Low, 0.),
                mana: AttributeValue::new(max_mana.0, AttributeQuality::Low, 0.),
                defence: AttributeValue::new(defence.0, AttributeQuality::Low, 0.),
                crit_chance: AttributeValue::new(crit_chance.0, AttributeQuality::Low, 0.),
                crit_damage: AttributeValue::new(crit_damage.0, AttributeQuality::Low, 0.),
                bonus_damage: AttributeValue::new(bonus_damage.0, AttributeQuality::Low, 0.),
                mana_regen: AttributeValue::new(mana_regen.0, AttributeQuality::Low, 0.),
                healing: AttributeValue::new(healing.0, AttributeQuality::Low, 0.),
                thorns: AttributeValue::new(thorns.0, AttributeQuality::Low, 0.),
                dodge: AttributeValue::new(dodge.0, AttributeQuality::Low, 0.),
                speed: AttributeValue::new(speed.0, AttributeQuality::Low, 0.),
                xp_rate: AttributeValue::new(xp_rate_bonus.0, AttributeQuality::Low, 0.),
                loot_rate: AttributeValue::new(loot_rate_bonus.0, AttributeQuality::Low, 0.),
                size: AttributeValue::new(size.0, AttributeQuality::Low, 0.),
                skill_power: AttributeValue::new(skill_power.0, AttributeQuality::Low, 0.),
                lifesteal: AttributeValue::new(lifesteal.0, AttributeQuality::Low, 0.),
                pickup_range: AttributeValue::new(pickup_range.0, AttributeQuality::Low, 0.),
                attack_speed: AttributeValue::new(attack_speed.0, AttributeQuality::Low, 0.),
                ..Default::default()
            }
            .get_stats_summary(curr_health.0, curr_mana.0, None, None);
            attributes.push(skills.poison_chance_stat_summary());

            // Hitbox is centered above "Try Again"; place tooltip up and to the left so it stays on-screen.
            let tooltip_pos = Vec3::new(-155., 46., 2.);
            let tooltip_e = spawn_stats_tooltip_at(
                &mut commands,
                &graphics,
                &asset_server,
                panel_e.unwrap(),
                tooltip_pos,
                &attributes,
            );
            commands.entity(tooltip_e).insert(GameOverStatsTooltip);
        }
    } else {
        for e in existing_tooltips.iter() {
            commands.entity(e).despawn();
        }
    }
}
