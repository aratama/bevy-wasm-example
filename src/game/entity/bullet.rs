use crate::game::world::wall::{BreakWallEvent, WallCollider};

use super::super::constant::{BULLET_GROUP, ENEMY_GROUP, WALL_GROUP};
use super::super::states::GameState;
use super::super::{asset::GameAssets, audio::play_se};
use super::book_shelf::BookShelf;
use super::enemy::Enemy;
use bevy::prelude::*;
use bevy_aseprite_ultra::prelude::{Aseprite, AsepriteSliceBundle};
use bevy_light_2d::light::PointLight2d;
use bevy_particle_systems::{
    ColorOverTime, JitteredValue, ParticleBurst, ParticleSystem, ParticleSystemBundle, Playing,
};
use bevy_rapier2d::prelude::*;
use std::collections::HashSet;

const SLICE_NAME: &str = "bullet";

static BULLET_Z: f32 = 10.0;

static BULLET_IMPULSE: f32 = 20000.0;

#[derive(Component, Reflect)]
pub struct Bullet {
    life: u32,
    damage: i32,
    impulse: f32,
}

#[derive(Bundle)]
pub struct BulletBundle {
    name: Name,
    bullet: Bullet,
    transform: Transform,
}

pub fn add_bullet(
    commands: &mut Commands,
    aseprite: Handle<Aseprite>,
    position: Vec2,
    velocity: Vec2,
) {
    commands.spawn((
        Name::new("bullet"),
        StateScoped(GameState::InGame),
        Bullet {
            life: 240,
            damage: 1,
            impulse: BULLET_IMPULSE,
        },
        AsepriteSliceBundle {
            aseprite,
            slice: SLICE_NAME.into(),
            transform: Transform::from_xyz(position.x, position.y, BULLET_Z)
                * Transform::from_rotation(Quat::from_rotation_z(velocity.to_angle())), // .looking_to(velocity.extend(BULLET_Z), Vec3::Z)
            ..default()
        },
        (
            Velocity {
                linvel: velocity,
                angvel: 0.0,
            },
            KinematicCharacterController::default(),
            RigidBody::KinematicVelocityBased,
            // 弾丸が大きくなると衝突時の位置の精度が悪化するので小さくしてあります
            Collider::ball(1.0),
            GravityScale(0.0),
            // https://rapier.rs/docs/user_guides/bevy_plugin/colliders#active-collision-types
            ActiveCollisionTypes::default() | ActiveCollisionTypes::KINEMATIC_STATIC,
            ActiveEvents::COLLISION_EVENTS,
            Sleeping::disabled(),
            // Ccd::enabled(),
            // https://rapier.rs/docs/user_guides/bevy_plugin/colliders#collision-groups-and-solver-groups
            CollisionGroups::new(BULLET_GROUP, WALL_GROUP | ENEMY_GROUP),
        ),
        PointLight2d {
            radius: 50.0,
            intensity: 1.0,
            falloff: 10.0,
            color: Color::hsl(245.0, 1.0, 0.6),
            ..default()
        },
    ));
}

pub fn update_bullet(
    mut commands: Commands,
    mut bullet_query: Query<(Entity, &mut Bullet, &Transform, &Velocity)>,
    mut enemy_query: Query<(&mut Enemy, &mut ExternalImpulse)>,
    mut bookshelf_query: Query<&mut BookShelf>,
    assets: Res<GameAssets>,

    mut collision_events: EventReader<CollisionEvent>,

    wall_collider_query: Query<Entity, With<WallCollider>>,
    mut break_wall_events: EventWriter<BreakWallEvent>,
    rapier_context: Res<RapierContext>,
) {
    // 弾丸のライフタイムを減らし、ライフタイムが尽きたら削除
    for (entity, mut bullet, _, _) in bullet_query.iter_mut() {
        bullet.life -= 1;
        if bullet.life <= 0 {
            commands.entity(entity).despawn();
        }
    }

    let mut despownings: HashSet<Entity> = HashSet::new();

    for collision_event in collision_events.read() {
        match collision_event {
            CollisionEvent::Started(a, b, _) => {
                if !process_bullet_event(
                    &mut commands,
                    &assets,
                    &mut bullet_query,
                    &mut enemy_query,
                    &mut bookshelf_query,
                    &mut despownings,
                    &a,
                    &b,
                    &wall_collider_query,
                    &mut break_wall_events,
                    &rapier_context,
                ) {
                    process_bullet_event(
                        &mut commands,
                        &assets,
                        &mut bullet_query,
                        &mut enemy_query,
                        &mut bookshelf_query,
                        &mut despownings,
                        &b,
                        &a,
                        &wall_collider_query,
                        &mut break_wall_events,
                        &rapier_context,
                    );
                }
            }
            _ => {}
        }
    }
}

fn process_bullet_event(
    mut commands: &mut Commands,
    assets: &Res<GameAssets>,
    query: &Query<(Entity, &mut Bullet, &Transform, &Velocity)>,
    enemies: &mut Query<(&mut Enemy, &mut ExternalImpulse)>,
    bookshelf_query: &mut Query<&mut BookShelf>,
    respownings: &mut HashSet<Entity>,
    a: &Entity,
    b: &Entity,
    wall_collider_query: &Query<Entity, With<WallCollider>>,
    break_wall_events: &mut EventWriter<BreakWallEvent>,
    rapier_context: &Res<RapierContext>,
) -> bool {
    // for contact_pair in rapier_context.contact_pairs_with(*a) {
    //     println!(
    //         "has_any_active_contact {:?}",
    //         contact_pair.has_any_active_contact()
    //     );
    // }
    // for contact_pair in rapier_context.contact_pairs_with(*b) {
    //     println!(
    //         "has_any_active_contact {:?}",
    //         contact_pair.has_any_active_contact()
    //     );
    // }
    // false
    // if let Some(contact_pair) = rapier_context.intersection_pair(*b, *a) {
    //     for manifold in contact_pair.manifolds() {
    //         if let Some(contact_view) = manifold.point(0) {
    //             println!("dist: {:?}", contact_view.dist());
    //         }
    //     }
    // }

    if let Ok((bullet_entity, bullet, bullet_transform, bullet_velocity)) = query.get(*a) {
        let bullet_position = bullet_transform.translation.truncate();

        // 弾丸が壁の角に当たった場合、衝突イベントが同時に複数回発生するため、
        // すでにdespownしたentityに対して再びdespownしてしまうことがあり、
        // 警告が出るのを避けるため、処理済みのentityを識別するセットを使っています
        // https://github.com/bevyengine/bevy/issues/5617
        if !respownings.contains(&bullet_entity) {
            respownings.insert(bullet_entity.clone());
            commands.entity(bullet_entity).despawn();
            spawn_particle_system(&mut commands, bullet_position);
            if let Ok((mut enemy, mut impilse)) = enemies.get_mut(*b) {
                // 弾丸が敵に衝突したとき
                enemy.life -= bullet.damage;
                impilse.impulse += bullet_velocity.linvel.normalize_or_zero() * bullet.impulse;
                play_se(&mut commands, assets.dageki.clone());
            } else if let Ok(mut bookshelf) = bookshelf_query.get_mut(*b) {
                // 弾丸が本棚に衝突したとき
                // TODO: この調子で破壊可能オブジェクトを増やすと、システムの引数やifの分岐が増えてしまう
                // Breakableコンポーネントにしてまとめる？
                // でも破壊したときの効果が物体によって異なるのでまとめられない？
                bookshelf.life -= bullet.damage;
                play_se(&mut commands, assets.dageki.clone());
            } else if let Ok(_) = wall_collider_query.get(*b) {
                // 弾丸が壁に衝突したとき

                // TODO
                // 衝突した点を取得しようとしたものの、正確な値がうまく取得できない
                // 弾丸が速くなると接触位置が数ピクセルずれることがある
                // https://rapier.rs/docs/user_guides/bevy_plugin/advanced_collision_detection#the-contact-graph
                if let Some(item) = rapier_context.contact_pair(bullet_entity, *b) {
                    if let Some((contact_view, _)) = item.find_deepest_contact() {
                        let local_p1 = contact_view.point(0).unwrap().local_p1();
                        let local_p2 = contact_view.point(0).unwrap().local_p2();
                        let body1 = contact_view.rigid_body1().unwrap();
                        let position = if body1.index() == bullet_entity.index() {
                            bullet_position - local_p1
                        } else {
                            bullet_position - local_p2
                        };
                        // println!("bullet position {:?}", bullet_position);
                        // println!("local_p1 {:?}", local_p1);
                        // println!("local_p2 {:?}", local_p2);
                        // println!(
                        //     "bullet_position - local_p1 {:?}",
                        //     bullet_position - local_p1
                        // );
                        // println!(
                        //     "bullet_position - local_p2 {:?}",
                        //     bullet_position - local_p2
                        // );
                        // println!("hit position {:?}", position);
                        // break_wall_events.send(BreakWallEvent { position });
                        // play_se(&mut commands, assets.shibafu.clone());
                    }
                }

                // 仕方ないので、弾丸の位置から近い壁を破壊する方法で凌ぐ
                break_wall_events.send(BreakWallEvent {
                    position: bullet_position,
                });
            } else {
                play_se(&mut commands, assets.shibafu.clone());
            }
            true
        } else {
            false
        }
    } else {
        false
    }
}

fn spawn_particle_system(commands: &mut Commands, position: Vec2) {
    commands
        // Add the bundle specifying the particle system itself.
        .spawn((
            Name::new("particle system"),
            StateScoped(GameState::InGame),
            ParticleSystemBundle {
                transform: Transform::from_translation(position.extend(BULLET_Z)),
                particle_system: ParticleSystem {
                    spawn_rate_per_second: 0.0.into(),
                    max_particles: 100,
                    initial_speed: JitteredValue::jittered(50.0, -50.0..50.0),
                    lifetime: JitteredValue::jittered(0.2, -0.05..0.05),
                    color: ColorOverTime::Constant(Color::WHITE),
                    bursts: vec![ParticleBurst {
                        // このシステムのスケジュールをUpdate意外に設定し、このtimeを0.0にすると、
                        // パーティクルシステムを設置してそのGlobalTransformが更新される前にパーティクルが生成されてしまうため、
                        // パーティクルの発生位置が原点になってしまうことに注意
                        // 0.1くらいにしておくと0.0ではないので大丈夫っぽい
                        time: 0.1,
                        count: 20,
                    }],
                    system_duration_seconds: 0.2,
                    ..ParticleSystem::oneshot()
                },
                ..ParticleSystemBundle::default()
            },
            Playing,
            PointLight2d {
                radius: 50.0,
                intensity: 1.0,
                falloff: 10.0,
                color: Color::hsl(245.0, 1.0, 0.6),
                ..default()
            },
        ));
}

pub struct BulletPlugin;

impl Plugin for BulletPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            update_bullet.run_if(in_state(GameState::InGame)),
        );
        app.register_type::<Bullet>();
    }
}
