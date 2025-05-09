use bevy::animation::{AnimationClip, AnimationPlayer, AnimationTargetId};
use bevy::prelude::*;
use bevy_animation::{AnimationTarget, animated_field};

// Holds information about the animation we programmatically create.
struct AnimationInfo {
    // The name of the animation target (in this case, the text).
    target_name: Name,
    // The ID of the animation target, derived from the name.
    target_id: AnimationTargetId,
    // The animation graph asset.
    graph: Handle<AnimationGraph>,
    // The index of the node within that graph.
    node_index: AnimationNodeIndex,
}

// The entry point.
fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        // Note that we don't need any systems other than the setup system,
        // because Bevy automatically updates animations every frame.
        .add_systems(Startup, setup)
        .add_systems(Update, print_transform)
        .run();
}

impl AnimationInfo {
    // Programmatically creates the UI animation.
    fn create(
        animation_graphs: &mut Assets<AnimationGraph>,
        animation_clips: &mut Assets<AnimationClip>,
    ) -> AnimationInfo {
        // Create an ID that identifies the text node we're going to animate.
        let animation_target_name = Name::new("Text");
        let animation_target_id = AnimationTargetId::from_name(&animation_target_name);

        // Allocate an animation clip.
        let mut animation_clip = AnimationClip::default();

        // Create a curve that animates font size.
        animation_clip.add_curve_to_target(
            animation_target_id,
            AnimatableCurve::new(
                animated_field!(Transform::translation),
                UnevenSampleAutoCurve::new([0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0].into_iter().zip([
                    Vec3::new(0.0, 0.0, 0.0),
                    Vec3::new(0.0, 0.0, 2.0),
                    Vec3::new(0.0, 2.0, 0.0),
                    Vec3::new(0.0, 0.0, 2.0),
                    Vec3::new(0.0, 2.0, 0.0),
                    Vec3::new(0.0, 0.0, 2.0),
                    Vec3::new(0.0, 2.0, 0.0),
                ]))
                .expect(
                    "should be able to build translation curve because we pass in valid samples",
                ),
            ),
        );

        let animation_clip_handle = animation_clips.add(animation_clip);

        // Create an animation graph with that clip.
        let (animation_graph, animation_node_index) =
            AnimationGraph::from_clip(animation_clip_handle);
        let animation_graph_handle = animation_graphs.add(animation_graph);

        AnimationInfo {
            target_name: animation_target_name,
            target_id: animation_target_id,
            graph: animation_graph_handle,
            node_index: animation_node_index,
        }
    }
}

// Creates all the entities in the scene.
fn setup(
    mut commands: Commands,
    mut animation_graphs: ResMut<Assets<AnimationGraph>>,
    mut animation_clips: ResMut<Assets<AnimationClip>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Create the animation.
    let AnimationInfo {
        target_name: animation_target_name,
        target_id: animation_target_id,
        graph: animation_graph,
        node_index: animation_node_index,
    } = AnimationInfo::create(&mut animation_graphs, &mut animation_clips);

    // Build an animation player that automatically plays the UI animation.
    let mut animation_player = AnimationPlayer::default();
    animation_player.play(animation_node_index).repeat();

    // Build the UI. We have a parent node that covers the whole screen and
    // contains the `AnimationPlayer`, as well as a child node that contains the
    // text to be animated.
    let material = StandardMaterial {
        base_color: Color::srgb(0.5, 0.7, 0.6),
        ..default()
    };
    let material_h = materials.add(material);
    let player = commands
        .spawn((
            animation_player,
            Visibility::Visible,
            Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
            Transform::from_xyz(0.0, 0.5, 0.0),
            MeshMaterial3d(material_h),
            AnimationGraphHandle(animation_graph.clone()),
            animation_target_name,
            // --- NOTE: Verify SpatialBundle is available ---
        ))
        .id();
    commands.entity(player).insert(AnimationTarget {
        id: animation_target_id,
        player,
    });
    commands.spawn((
        Camera3d { ..default() },
        Transform::from_xyz(-2.5, 4.5, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    // --- NOTE: Verify PointLightBundle is available ---
    commands.spawn((PointLight {
        shadows_enabled: true,
        ..default()
    },));
}

fn print_transform(query: Query<&Transform, With<Text>>) {
    for thing in query.iter() {
        println!("transform: {:?}", thing);
    }
}
