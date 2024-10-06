// use super::constant::CRATE_NAME;
// use bevy::asset::io::*;
use bevy::asset::*;
use bevy::audio::PlaybackMode;
use bevy::prelude::*;
use bevy_aseprite_ultra::prelude::AsepriteSliceBundle;
use bevy_particle_systems::{
    ColorOverTime, JitteredValue, ParticleBurst, ParticleSystem, ParticleSystemBundle, Playing,
};
use bevy_rapier2d::prelude::*;
// use std::path::Path;

const ASEPRITE_PATH: &str = "asset.aseprite";

const SLICE_NAME: &str = "bullet";

static BULLET_Z: f32 = 10.0;

#[derive(Component, Reflect)]
pub struct Bullet {
    life: u32,
}

#[derive(Bundle)]
pub struct BulletBundle {
    name: Name,
    bullet: Bullet,
    transform: Transform,
}

pub fn add_bullet(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    position: Vec2,
    velocity: Vec2,
) {
    // TODO:
    // 現状ではアセットの埋め込みはしていません
    // 埋め込みができない理由は、world.rs のコメントに記載しています
    //
    // // https://bevyengine.org/examples/assets/embedded-asset/
    // アセットのパスは URL のような形式になっており、
    // プロトコルは embedded、それ以下のパスは crate 名とアセットのパスになります
    // embedded_asset 側の設定で、パスの game は省略されます
    // 実際のパスは Window では例えば、 embedded://my_bevy_game\asset/asset.aseprite になります
    // let path = Path::new(CRATE_NAME).join("asset/asset.aseprite");
    // let asset_path = AssetPath::from_path(&path).with_source(AssetSourceId::from("embedded"));

    commands.spawn((
        Bullet { life: 120 },
        AsepriteSliceBundle {
            // aseprite: asset_server.load(asset_path),
            aseprite: asset_server.load(ASEPRITE_PATH),
            slice: SLICE_NAME.into(),
            transform: Transform::from_xyz(position.x, position.y, BULLET_Z)
                * Transform::from_rotation(Quat::from_rotation_z(velocity.to_angle())), // .looking_to(velocity.extend(BULLET_Z), Vec3::Z)
            ..default()
        },
        Velocity {
            linvel: velocity,
            angvel: 0.0,
        },
        KinematicCharacterController::default(),
        RigidBody::KinematicVelocityBased,
        Collider::ball(5.0),
        GravityScale(0.0),
        Sensor,
        // https://rapier.rs/docs/user_guides/bevy_plugin/colliders#active-collision-types
        ActiveCollisionTypes::default() | ActiveCollisionTypes::KINEMATIC_STATIC,
        ActiveEvents::COLLISION_EVENTS,
        Sleeping::disabled(),
        Ccd::enabled(),
    ));
}

pub fn update_bullet(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Bullet, &Transform)>,
    mut collision_events: EventReader<CollisionEvent>,
    asset_server: Res<AssetServer>,
) {
    for (entity, mut bullet, _) in query.iter_mut() {
        bullet.life -= 1;
        if bullet.life <= 0 {
            commands.entity(entity).despawn();
        }
    }
    for collision_event in collision_events.read() {
        match collision_event {
            CollisionEvent::Started(a, b, _) => {
                // 弾丸は何かに接触した時点で消滅する
                if let Ok((_, _, bul)) = query.get(*a) {
                    commands.entity(*a).despawn();
                    spawn_particle_system(&mut commands, bul.translation.truncate());
                    play_despown_bullet_se(&asset_server, &mut commands);
                }
                if let Ok((_, _, bul)) = query.get(*b) {
                    commands.entity(*b).despawn();
                    spawn_particle_system(&mut commands, bul.translation.truncate());
                    play_despown_bullet_se(&asset_server, &mut commands);
                }
            }
            _ => {}
        }
    }
}

fn play_despown_bullet_se(asset_server: &Res<AssetServer>, commands: &mut Commands) {
    commands.spawn(AudioBundle {
        source: asset_server.load("shibafu.ogg"),
        settings: PlaybackSettings {
            mode: PlaybackMode::Despawn,
            ..default()
        },
        ..default()
    });
}

fn spawn_particle_system(commands: &mut Commands, position: Vec2) {
    commands
        // Add the bundle specifying the particle system itself.
        .spawn((
            ParticleSystemBundle {
                transform: Transform::from_translation(position.extend(BULLET_Z)),
                particle_system: ParticleSystem {
                    spawn_rate_per_second: 0.0.into(),
                    max_particles: 100,
                    initial_speed: JitteredValue::jittered(50.0, -50.0..50.0),
                    lifetime: JitteredValue::jittered(0.2, -0.05..0.05),
                    color: ColorOverTime::Constant(Color::WHITE),
                    bursts: vec![ParticleBurst {
                        time: 0.0,
                        count: 20,
                    }],
                    ..ParticleSystem::oneshot()
                },
                ..ParticleSystemBundle::default()
            },
            Playing,
        ));
}

pub struct BulletPlugin;

impl Plugin for BulletPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, update_bullet);
    }
}
