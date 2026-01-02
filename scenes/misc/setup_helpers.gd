extends Node
class_name SetupHelpers

# checks if the scene is capable of code execution, probably not foolproof
static func check_safe(root:SceneState) -> bool:
	# check for scripts
	for idx:int in range(root.get_node_count()):
		for property_idx:int in range(root.get_node_property_count(idx)):
			if root.get_node_property_name(idx, property_idx) == "script":
				if root.get_node_property_value(idx, property_idx) != null:
					return false
	return true

# todo: replace with iterative callback version from cck
static func get_node_and_children_recursive(root:Node) -> Array[Node]:
	var nodes:Array[Node]
	nodes.append(root)
	for node:Node in root.get_children():
		nodes.append_array(get_node_and_children_recursive(node))
	return nodes

static func setup_world(root:Node) -> WorldController:
	var spawnpoint:Node3D
	for node:Node in get_node_and_children_recursive(root):
		# todo: move node setups into its own function
		if node.has_meta("Spawnpoint"):
			spawnpoint = node
		if node is Camera3D:
			node.queue_free()
	
	var world:WorldController = WorldController.setup(spawnpoint)
	world.add_child(root)
	
	return world

# goes through the avatar scene looking for stubs and replaces them with the corrosponding scenes
static func setup_avatar(root:Node, player:Player) -> void:
	var combined_aabb:AABB = AABB(Vector3(0, 0, 0), Vector3(0.1, 0.1, 0.1))
	var nodes:Array[Node] = get_node_and_children_recursive(root)
	for node:Node in nodes:
		if node is VisualInstance3D:
			var aabb:AABB = (node as VisualInstance3D).get_aabb().abs()
			combined_aabb.merge(aabb)
		if node.has_meta("IKMarker") and node is Skeleton3D:
			var ik:AvatarIK = preload("res://scenes/player/avatar/godot_ik.tscn").instantiate()
			var values:Dictionary = node.get_meta("IKMarker")
			# todo: check marker is valid
			node.add_child(ik)
			ik.head_bone = values["head_bone"]
			ik.left_arm_bone = values["left_arm_bone"]
			ik.right_arm_bone = values["right_arm_bone"]
			ik.left_leg_bone = values["left_leg_bone"]
			ik.right_leg_bone = values["right_leg_bone"]
			ik.spine_bone = values["spine_bone"]
			ik.hip_bone = values["hip_bone"]
			ik.head_target = node.get_child(values["head_target"])
			ik.left_arm_target = node.get_child(values["left_arm_target"])
			ik.right_arm_target = node.get_child(values["right_arm_target"])
			ik.spine_target = node.get_child(values["spine_target"])
			ik.hip_target = node.get_child(values["hip_target"])
			ik.setup(player.is_local)
			player.head_ik_target = ik.head.target
			player.left_arm_ik_target = ik.left_arm.target
			player.right_arm_ik_target = ik.right_arm.target
			@warning_ignore("unsafe_call_argument", "unsafe_property_access")
			player.head_view_offset = node.get_child(values["head_view"]).position - node.get_child(values["head_target"]).position
	(player.collider.shape as CapsuleShape3D).radius = maxf(combined_aabb.size.x, combined_aabb.size.z)
	(player.collider.shape as CapsuleShape3D).height = combined_aabb.size.y
	player.collider.position = combined_aabb.position
