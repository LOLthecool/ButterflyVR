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
	
	const FOV:float = 75.0
	var aabb:AABB = AABB()
	for child:Node in SetupHelpers.get_node_and_children_recursive(avatar):
		if child is VisualInstance3D:
			(child as VisualInstance3D).layers = 4096 # layer 13
			aabb = aabb.merge((child as VisualInstance3D).get_aabb())
	var pos:Vector3 = aabb.position + (aabb.size / 2)
	# todo: something is broken here.
	# this should place the camera as close as possible to the object,
	# while leaving everything visible.
	# right now it places the camers way too far away and i have no idea why
	# the idea was to treat the camera positioning like a right angle triangle 
	# with fov / 2 being the angle and aabb.size.y / 2 being the opposite side
	# solving for the adjacent side should give us the camera distance but it dosent
	pos.z += (maxf(aabb.size.x, aabb.size.y) / 2) / abs(tan(FOV / 2)) * 1.1
	
	camera.position = pos

func _process(delta: float) -> void:
	rotation.y += 0.2 * delta
