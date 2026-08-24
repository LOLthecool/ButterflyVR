@tool
extends Node3D
class_name Previewer

@export var viewport: SubViewport
@export var camera: Camera3D
var mutex: Mutex = Mutex.new()


func create_preview(uuid: UUID) -> void:
	MiscHelpers.await_lock_mutex(mutex)
	for child: Node in get_children():
		child.queue_free()

	rotation.y = 0

	var avatar_scene: PackedScene = await GlobalDownloadHandler.get_object(
		uuid,
		TypeHelper.ObjectType.avatar,
	)

	if !avatar_scene:
		push_warning("failed to load avatar %s for preview" % uuid)
		return

	var avatar: Node = avatar_scene.instantiate()
	add_child(avatar)

	var aabb: AABB = AABB()
	for child: Node in SetupHelpers.get_node_and_children_recursive(avatar):
		if child is VisualInstance3D:
			(child as VisualInstance3D).layers = 4096 # layer 13
			aabb = aabb.merge((child as VisualInstance3D).get_aabb())
	var pos: Vector3 = aabb.position + (aabb.size / 2)

	var candidate1: float = (
		(aabb.size.x / 2) / absf(tan(deg_to_rad(camera.get_camera_projection().get_fov() / 2)))
	) * 1.1
	var fovy: float = Projection.get_fovy(
		camera.get_camera_projection().get_fov() / 2,
		1 / camera.get_camera_projection().get_aspect(),
	)
	var candidate2: float = ((aabb.size.y / 2) / absf(tan(deg_to_rad(fovy / 2)))) * 1.1

	pos.z += maxf(candidate1, candidate2)
	pos.z = -pos.z
	camera.position = pos

	await get_tree().physics_frame

	mutex.unlock()


func _process(delta: float) -> void:
	rotation.y += 0.4 * delta
