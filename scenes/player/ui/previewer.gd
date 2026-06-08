@tool
extends Node3D
class_name Previewer

@export var viewport:SubViewport
@export var camera:Camera3D

func create_preview(uuid:UUID) -> void:
	if get_child_count() != 0:
		get_child(0).queue_free()
	
	rotation.y = 0
	
	var avatar_scene:PackedScene = await GlobalDownloadHandler.get_object(
			uuid, LRUCache.ObjectType.avatar)
	
	if !avatar_scene:
		push_error("failed to load avatar %s for preview" % uuid)
		return
	
	if !SetupHelpers.check_safe(avatar_scene.get_state()):
		push_error("tried to preview unsafe avatar!")
		return
	
	var avatar:Node = avatar_scene.instantiate()
	add_child(avatar)
	
	var aabb:AABB = AABB()
	for child:Node in SetupHelpers.get_node_and_children_recursive(avatar):
		if child is VisualInstance3D:
			(child as VisualInstance3D).layers = 4096 # layer 13
			aabb = aabb.merge((child as VisualInstance3D).get_aabb())
	var pos:Vector3 = aabb.position + (aabb.size / 2)
	
	var candidate1:float = (
			(aabb.size.x / 2) / absf(tan(deg_to_rad(camera.get_camera_projection().get_fov() / 2)))) * 1.1
	var fovy:float = Projection.get_fovy(
			camera.get_camera_projection().get_fov() / 2, 
			1 / camera.get_camera_projection().get_aspect())
	var candidate2:float = ((aabb.size.y / 2) / absf(tan(deg_to_rad(fovy / 2)))) * 1.1
	
	pos.z += maxf(candidate1, candidate2)
	
	camera.position = pos

func _process(delta: float) -> void:
	rotation.y += 0.2 * delta
