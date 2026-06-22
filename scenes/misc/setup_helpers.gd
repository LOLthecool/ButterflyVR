extends Node
class_name SetupHelpers

class SetupState:
	var state:Dictionary[String, Variant]

# checks if the scene is doing anything definetly bad
static func check_safe(root:SceneState) -> bool:
	for idx:int in range(root.get_node_count()):
		for group:String in root.get_node_groups(idx):
			if !(group.begins_with("_") or group.begins_with("_cck")):
				return false
		# check for scripts
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

static func get_cck_markers() -> Array[CCKMarker]:
	var class_paths:Array[String] = []
	for global_class:Dictionary in ProjectSettings.get_global_class_list():
		if global_class["base"] == "CCKMarker":
			class_paths.push_back(global_class["path"])
	var classes:Array[CCKMarker] = []
	classes.assign(class_paths.map(
			func(path:String) -> CCKMarker: 
				@warning_ignore("unsafe_cast")
				return (load(path) as GDScript).new() as CCKMarker
				))
	return classes

static func setup_world(root:Node) -> WorldController:
	const blacklisted_nodes:Array[String] = []
	
	var state:SetupState = SetupState.new()
	var cck_markers:Array[CCKMarker] = get_cck_markers()
	
	for node:Node in get_node_and_children_recursive(root):
		if node.get_class() in blacklisted_nodes:
			node.queue_free()
			continue
		
		for marker:CCKMarker in cck_markers:
			if !marker.is_allowed_on(LRUCache.ObjectType.world):
				continue
			if marker.supports_multiple_copies():
				for meta_item:StringName in node.get_meta_list():
					if meta_item.split(":")[0] == marker.get_name():
						var values:Dictionary[String, Variant] = {}
						@warning_ignore("unsafe_cast")
						values.assign(node.get_meta(meta_item) as Dictionary)
						@warning_ignore("unsafe_cast")
						marker.setup(values, node, state)
			else:
				if node.has_meta(marker.get_name()):
					var values:Dictionary[String, Variant] = {}
					@warning_ignore("unsafe_cast")
					values.assign(node.get_meta(marker.get_name()) as Dictionary)
					@warning_ignore("unsafe_cast")
					marker.setup(values, node, state)
	
	var world:WorldController
	if state.state.has("spawnpoint"):
		@warning_ignore("unsafe_cast")
		world = WorldController.setup(state.state["spawnpoint"] as Node3D)
	else:
		var spawn_node:Node3D = Node3D.new()
		root.add_child(spawn_node)
		world = WorldController.setup(spawn_node)
	world.add_child(root)
	
	return world

static func setup_avatar(root:Node, player:Player) -> void:
	const blacklisted_nodes:Array[String] = []
	
	var state:SetupState = SetupState.new()
	var cck_markers:Array[CCKMarker] = get_cck_markers()
	var combined_aabb:AABB = AABB(Vector3(0, 0, 0), Vector3(0.1, 0.1, 0.1))
	var nodes:Array[Node] = get_node_and_children_recursive(root)
	
	state.state["is_local"] = player.is_local
	
	for node:Node in nodes:
		if node.get_class() in blacklisted_nodes:
			node.queue_free()
			continue
		
		if node is VisualInstance3D:
			var aabb:AABB = (node as VisualInstance3D).get_aabb().abs()
			combined_aabb.merge(aabb)
		
		
		for marker:CCKMarker in cck_markers:
			if !marker.is_allowed_on(LRUCache.ObjectType.avatar):
				continue
			if marker.supports_multiple_copies():
				for meta_item:StringName in node.get_meta_list():
					if meta_item.split(":")[0] == marker.get_name():
						@warning_ignore("unsafe_cast")
						marker.setup(
								node.get_meta(meta_item) as Dictionary, 
								node, state)
			else:
				if node.has_meta(marker.get_name()):
					@warning_ignore("unsafe_cast")
					marker.setup(
							node.get_meta(marker.get_name()) as Dictionary, 
							node, state)
	
	if state.state.has("ik_values"):
		player.head_ik_target = state.state["ik_values"]["head_target"]
		player.head_view_offset = state.state["ik_values"]["head_view"]
	
	player.position.y -= (player.collider.shape as CapsuleShape3D).height / 2
	
	player.collider.shape = CapsuleShape3D.new()
	
	# godot dosent let us create invalid capsule shapes even temporarily so we need to modify in the correct order
	# there is also no way to construct a capsule shape in one go
	if maxf(combined_aabb.size.x, combined_aabb.size.z) > (player.collider.shape as CapsuleShape3D).height / 2:
		(player.collider.shape as CapsuleShape3D).height = combined_aabb.size.y
		(player.collider.shape as CapsuleShape3D).radius = maxf(combined_aabb.size.x, combined_aabb.size.z)
	else:
		(player.collider.shape as CapsuleShape3D).radius = maxf(combined_aabb.size.x, combined_aabb.size.z)
		(player.collider.shape as CapsuleShape3D).height = combined_aabb.size.y
	
	player.collider.position = combined_aabb.position
	
	player.position.y += (player.collider.shape as CapsuleShape3D).height / 2
