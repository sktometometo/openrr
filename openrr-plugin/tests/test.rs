use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};

use arci::{
    gamepad::GamepadEvent, BaseVelocity, DummyGamepad, DummyJointTrajectoryClient,
    DummyLocalization, DummyMoveBase, DummyNavigation, DummySpeaker, DummyTransformResolver,
    Gamepad, Isometry2, JointTrajectoryClient, Localization, MoveBase, Navigation, Speaker,
    TrajectoryPoint, TransformResolver, Vector2,
};
use assert_approx_eq::assert_approx_eq;
use openrr_plugin::{
    GamepadProxy, JointTrajectoryClientProxy, LocalizationProxy, MoveBaseProxy, NavigationProxy,
    SpeakerProxy, TransformResolverProxy,
};

#[tokio::test]
async fn joint_trajectory_client() {
    let client = Arc::new(DummyJointTrajectoryClient::new(vec![
        "a".to_owned(),
        "b".to_owned(),
    ]));
    let proxy = JointTrajectoryClientProxy::new(client.clone());

    assert_eq!(proxy.joint_names(), vec!["a", "b"]);
    let pos = client.current_joint_positions().unwrap();
    assert_eq!(pos.len(), 2);
    assert_approx_eq!(pos[0], 0.0);
    assert_approx_eq!(pos[1], 0.0);
    let pos = proxy.current_joint_positions().unwrap();
    assert_eq!(pos.len(), 2);
    assert_approx_eq!(pos[0], 0.0);
    assert_approx_eq!(pos[1], 0.0);
    proxy
        .send_joint_positions(vec![1.0, 2.0], Duration::from_secs(1))
        .unwrap()
        .await
        .unwrap();
    let pos2 = client.current_joint_positions().unwrap();
    assert_eq!(pos2.len(), 2);
    assert_approx_eq!(pos2[0], 1.0);
    assert_approx_eq!(pos2[1], 2.0);
    let pos2 = proxy.current_joint_positions().unwrap();
    assert_eq!(pos2.len(), 2);
    assert_approx_eq!(pos2[0], 1.0);
    assert_approx_eq!(pos2[1], 2.0);

    proxy
        .send_joint_trajectory(vec![
            TrajectoryPoint::new(vec![1.0, -1.0], Duration::from_secs(1)),
            TrajectoryPoint::new(vec![2.0, -3.0], Duration::from_secs(2)),
        ])
        .unwrap()
        .await
        .unwrap();
    assert_eq!(client.last_trajectory.lock().unwrap().len(), 2);
    let pos = client.current_joint_positions().unwrap();
    assert_eq!(pos.len(), 2);
    assert_approx_eq!(pos[0], 2.0);
    assert_approx_eq!(pos[1], -3.0);
    let pos = proxy.current_joint_positions().unwrap();
    assert_eq!(pos.len(), 2);
    assert_approx_eq!(pos[0], 2.0);
    assert_approx_eq!(pos[1], -3.0);
}

#[tokio::test]
async fn speaker() {
    let speaker = Arc::new(DummySpeaker::new());
    let proxy = SpeakerProxy::new(speaker.clone());

    assert_eq!(speaker.current_message(), "");
    proxy.speak("abc").unwrap().await.unwrap();
    assert_eq!(speaker.current_message(), "abc");
}

#[tokio::test]
async fn move_base() {
    let base = Arc::new(DummyMoveBase::new());
    let proxy = MoveBaseProxy::new(base.clone());

    let vel1 = base.current_velocity().unwrap();
    assert_approx_eq!(vel1.x, 0.0);
    assert_approx_eq!(vel1.y, 0.0);
    assert_approx_eq!(vel1.theta, 0.0);
    let vel1 = proxy.current_velocity().unwrap();
    assert_approx_eq!(vel1.x, 0.0);
    assert_approx_eq!(vel1.y, 0.0);
    assert_approx_eq!(vel1.theta, 0.0);
    proxy
        .send_velocity(&BaseVelocity::new(0.1, 0.2, -3.0))
        .unwrap();
    let vel2 = base.current_velocity().unwrap();
    assert_approx_eq!(vel2.x, 0.1);
    assert_approx_eq!(vel2.y, 0.2);
    assert_approx_eq!(vel2.theta, -3.0);
    let vel2 = proxy.current_velocity().unwrap();
    assert_approx_eq!(vel2.x, 0.1);
    assert_approx_eq!(vel2.y, 0.2);
    assert_approx_eq!(vel2.theta, -3.0);
}

#[tokio::test]
async fn navigation() {
    let nav = Arc::new(DummyNavigation::new());
    let proxy = NavigationProxy::new(nav.clone());

    proxy
        .send_goal_pose(
            Isometry2::new(Vector2::new(1.0, 2.0), 3.0),
            "",
            Duration::default(),
        )
        .unwrap()
        .await
        .unwrap();
    let pose = nav.current_goal_pose().unwrap();
    assert_approx_eq!(pose.translation.x, 1.0);
    assert_approx_eq!(pose.translation.y, 2.0);
    assert_approx_eq!(pose.rotation.angle(), 3.0);
    proxy.cancel().unwrap();
    assert!(nav.is_canceled());
}

#[tokio::test]
async fn localization() {
    let loc = Arc::new(DummyLocalization::new());
    let proxy = LocalizationProxy::new(loc);

    let pose = proxy.current_pose("").unwrap();
    assert_eq!(pose, pose.inverse()); // only identity mapping satisfies this
}

#[tokio::test]
async fn transform_resolver() {
    let resolver = Arc::new(DummyTransformResolver::default());
    let proxy = TransformResolverProxy::new(resolver);

    let transformation = proxy
        .resolve_transformation("", "", SystemTime::UNIX_EPOCH)
        .unwrap();
    assert_eq!(transformation, transformation.inverse())
}

#[tokio::test]
async fn gamepad() {
    let gamepad = Arc::new(DummyGamepad::with_all_events());
    let proxy = GamepadProxy::new(gamepad.clone());

    for expected in &gamepad.events {
        match (expected, proxy.next_event().await) {
            (GamepadEvent::ButtonPressed(expected), GamepadEvent::ButtonPressed(actual)) => {
                assert_eq!(*expected, actual);
            }
            (GamepadEvent::ButtonReleased(expected), GamepadEvent::ButtonReleased(actual)) => {
                assert_eq!(*expected, actual);
            }
            (
                GamepadEvent::AxisChanged(expected_axis, expected_val),
                GamepadEvent::AxisChanged(actual_axis, actual_val),
            ) => {
                assert_eq!(*expected_axis, actual_axis);
                assert_approx_eq!(*expected_val, actual_val);
            }
            (GamepadEvent::Unknown, GamepadEvent::Unknown) => {}
            (expected, actual) => panic!(
                "event mismatch, expected: {:?}, actual: {:?}",
                expected, actual
            ),
        }
    }
    proxy.stop();
    assert!(gamepad.is_stopped());
}
