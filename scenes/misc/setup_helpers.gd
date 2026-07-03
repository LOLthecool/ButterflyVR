extends Node
class_name SetupHelpers

class SetupState:
	var state:Dictionary[String, Variant]

# checks if the scene is capable of running code or doing anything else bad during instanciation
# (for example a gdscript can have a _init() function)
# other safety check can deferred until the setup_x functions or handled here
# checks handled in setup_x should be checks that are hard or impossible to do with a SceneState
static func check_safe(root:SceneState) -> bool:
	for idx:int in range(root.get_node_count()):
		if root.get_node_type(idx).begins_with("Editor"):
			return false
		for group:String in root.get_node_groups(idx):
			if !(group.begins_with("_") or group.begins_with("_cck")):
				return false
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
	var blacklisted_nodes:Array = [AnimationMixer, Window, HTTPRequest, MultiplayerSpawner, MultiplayerSynchronizer, StatusIndicator, APIHandler, APIHelper, AccountHandler, AgonesSDK, MessageHandler, ImageDownloadHandler, InstanceHandler, MessageManager, MovementHandler, NetNodeManager, NetworkedNode, PathHelper, PersistanceHandler, ServerHandler, ServerLoader, SetupHelpers, StringifyHelper, TypeHelper, WorldController, WorldHandler]
	
	var state:SetupState = SetupState.new()
	var cck_markers:Array[CCKMarker] = get_cck_markers()
	
	for node:Node in get_node_and_children_recursive(root):
		@warning_ignore("untyped_declaration")
		if blacklisted_nodes.any(func(blacklist_item) -> bool:
				return is_instance_of(node, blacklist_item)):
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
	var blacklisted_nodes:Array = [AnimationMixer, Window, HTTPRequest, MultiplayerSpawner, MultiplayerSynchronizer, StatusIndicator, APIHandler, APIHelper, AccountHandler, AgonesSDK, MessageHandler, ImageDownloadHandler, InstanceHandler, MessageManager, MovementHandler, NetNodeManager, NetworkedNode, PathHelper, PersistanceHandler, ServerHandler, ServerLoader, SetupHelpers, StringifyHelper, TypeHelper, WorldController, WorldHandler]
	
	var state:SetupState = SetupState.new()
	var cck_markers:Array[CCKMarker] = get_cck_markers()
	var combined_aabb:AABB = AABB(Vector3.ZERO, Vector3.ZERO)
	var nodes:Array[Node] = get_node_and_children_recursive(root)
	
	state.state["is_local"] = player.is_local
	
	for node:Node in nodes:
		@warning_ignore("untyped_declaration")
		if blacklisted_nodes.any(func(blacklist_item) -> bool:
				return is_instance_of(node, blacklist_item)):
			node.queue_free()
			continue
		
		if node is VisualInstance3D:
			if combined_aabb == AABB(Vector3.ZERO, Vector3.ZERO):
				combined_aabb = (node as VisualInstance3D).get_aabb()
			else:
				var aabb:AABB = (node as VisualInstance3D).get_aabb()
				combined_aabb = combined_aabb.merge(aabb)
		
		
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
	
	if combined_aabb == AABB(Vector3.ZERO, Vector3.ZERO):
		combined_aabb = AABB(Vector3.ZERO, Vector3(0.1, 0.4, 0.1))
	
	
	player.collider.shape = CapsuleShape3D.new()
	
	# godot dosent let us create invalid capsule shapes even temporarily so we need to modify in the correct order
	# there is also no way to construct a capsule shape in one go
	if maxf(combined_aabb.size.x, combined_aabb.size.z) > (player.collider.shape as CapsuleShape3D).height / 2:
		(player.collider.shape as CapsuleShape3D).height = combined_aabb.size.y
		(player.collider.shape as CapsuleShape3D).radius = maxf(combined_aabb.size.x, combined_aabb.size.z)
	else:
		(player.collider.shape as CapsuleShape3D).radius = maxf(combined_aabb.size.x, combined_aabb.size.z)
		(player.collider.shape as CapsuleShape3D).height = combined_aabb.size.y
	
	
	if state.state.has("collider_values"):
		var radius:float = state.state["collider_values"]["radius"]
		var height:float = state.state["collider_values"]["height"]
		
		radius = maxf((player.collider.shape as CapsuleShape3D).radius / 4, 
				minf((player.collider.shape as CapsuleShape3D).radius * 2, radius))
		
		height = maxf((player.collider.shape as CapsuleShape3D).height / 4, 
				minf((player.collider.shape as CapsuleShape3D).height * 2, height))
		
		if radius > height / 2:
			(player.collider.shape as CapsuleShape3D).height = height
			(player.collider.shape as CapsuleShape3D).radius = radius
		else:
			(player.collider.shape as CapsuleShape3D).radius = radius
			(player.collider.shape as CapsuleShape3D).height = height
	
	player.collider.position = combined_aabb.position + \
			(Vector3(
					combined_aabb.size.x, (player.collider.shape as CapsuleShape3D).height, combined_aabb.size.z) / 2)
	
	player.position.y += (player.collider.shape as CapsuleShape3D).height / 2
