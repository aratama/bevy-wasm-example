use super::remote::RemoteMessage;
use crate::asset::GameAssets;
use crate::constant::*;
use crate::entity::actor::Actor;
use crate::entity::bullet::{add_bullet, BULLET_RADIUS, BULLET_SPAWNING_MARGIN};
use crate::entity::witch::WITCH_COLLIDER_RADIUS;
use crate::gamepad::{get_direction, get_fire_trigger, MyGamepad};
use crate::states::GameState;
use bevy::core::FrameCount;
use bevy::prelude::*;
use bevy_rapier2d::prelude::*;
use bevy_simple_websocket::ClientMessage;
use rand::random;
use std::f32::consts::PI;

// 魔法の拡散
const BULLET_SCATTERING: f32 = 0.3;

// 魔法弾の速度
// pixels_per_meter が 100.0 に設定されているので、
// 200は1フレームに2ピクセル移動する速度です
const BULLET_SPEED: f32 = 200.0;

// 次の魔法を発射するまでの待機フレーム数
const BULLET_COOLTIME: i32 = 8;

// 一度に発射する弾丸の数
const BULLETS_PER_FIRE: u32 = 1;

/// 操作可能なプレイヤーキャラクターを表します
#[derive(Component)]
pub struct Player {
    pub last_idle_frame_count: FrameCount,
    pub last_ilde_x: f32,
    pub last_ilde_y: f32,
    pub last_idle_vx: f32,
    pub last_idle_vy: f32,
}

fn update_player(
    keys: Res<ButtonInput<KeyCode>>,
    mut player_query: Query<
        (
            &mut Actor,
            &mut Transform,
            &mut ExternalForce,
            &GlobalTransform,
            &mut Sprite,
        ),
        (With<Player>, Without<Camera2d>),
    >,
    mut camera_query: Query<&mut Transform, (With<Camera>, With<Camera2d>, Without<Player>)>,
    mut commands: Commands,
    assets: Res<GameAssets>,
    buttons: Res<ButtonInput<MouseButton>>,

    my_gamepad: Option<Res<MyGamepad>>,
    axes: Res<Axis<GamepadAxis>>,
    gamepad_buttons: Res<ButtonInput<GamepadButton>>,

    mut writer: EventWriter<ClientMessage>,
) {
    let force = 50000.0;

    let direction = get_direction(keys, axes, &my_gamepad);

    if let Ok((mut player, mut player_transform, mut player_force, _, mut player_sprite)) =
        player_query.get_single_mut()
    {
        player_transform.translation.z =
            ENTITY_LAYER_Z - player_transform.translation.y * Z_ORDER_SCALE;
        player_force.force = direction * force;

        if let Ok(mut camera_transform) = camera_query.get_single_mut() {
            camera_transform.translation.x +=
                (player_transform.translation.x - camera_transform.translation.x) * CAMERA_SPEED;
            camera_transform.translation.y +=
                (player_transform.translation.y - camera_transform.translation.y) * CAMERA_SPEED;

            // プレイヤーの向き
            let angle = player.pointer.to_angle();

            if angle < -PI * 0.5 || PI * 0.5 < angle {
                player_sprite.flip_x = true;
            } else {
                player_sprite.flip_x = false;
            }

            // 魔法の発射
            if get_fire_trigger(buttons, gamepad_buttons, &my_gamepad) && player.cooltime == 0 {
                let normalized = player.pointer.normalize();

                for _ in 0..BULLETS_PER_FIRE {
                    let angle_with_random = angle + (random::<f32>() - 0.5) * BULLET_SCATTERING;
                    let direction = Vec2::from_angle(angle_with_random);
                    let range = WITCH_COLLIDER_RADIUS + BULLET_RADIUS + BULLET_SPAWNING_MARGIN;
                    let bullet_position =
                        player_transform.translation.truncate() + range * normalized;
                    add_bullet(
                        &mut commands,
                        assets.asset.clone(),
                        bullet_position,
                        direction * BULLET_SPEED,
                        Some(player.uuid),
                    );
                    let serialized = bincode::serialize(&RemoteMessage::Fire {
                        uuid: player.uuid,
                        x: bullet_position.x,
                        y: bullet_position.y,
                        vx: direction.x * BULLET_SPEED,
                        vy: direction.y * BULLET_SPEED,
                    })
                    .unwrap();
                    writer.send(ClientMessage::Binary(serialized));
                }

                player.cooltime = BULLET_COOLTIME;
            } else {
                player.cooltime = (player.cooltime - 1).max(0);
            }
        }
    }
}

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            update_player.run_if(in_state(GameState::InGame)),
        );
    }
}