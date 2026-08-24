extends Node
class_name SetupHelpers


class SetupState:
	var state: Dictionary[String, Variant]


# todo: replace with iterative callback version from cck
static func get_node_and_children_recursive(root: Node) -> Array[Node]:
	var nodes: Array[Node]
	nodes.append(root)
	for node: Node in root.get_children():
		nodes.append_array(get_node_and_children_recursive(node))
	return nodes


static func get_cck_markers() -> Array[CCKMarker]:
	var class_paths: Array[String] = []
	for global_class: Dictionary in ProjectSettings.get_global_class_list():
		if global_class["base"] == "CCKMarker":
			class_paths.push_back(global_class["path"])
	var classes: Array[CCKMarker] = []
	classes.assign(
		class_paths.map(
			func(path: String) -> CCKMarker:
				@warning_ignore("unsafe_cast")
				return (load(path) as GDScript).new() as CCKMarker,
		)
	)
	return classes


static func setup_object(root: Node, type: TypeHelper.ObjectType, state: SetupState) -> void:
	var blacklisted_nodes: Array = [
		AnimationMixer,
		Window,
		HTTPRequest,
		MultiplayerSpawner,
		MultiplayerSynchronizer,
		StatusIndicator,
		APIHandler,
		APIHelper,
		AccountHandler,
		AgonesSDK,
		MessageHandler,
		ImageDownloadHandler,
		InstanceHandler,
		MessageManager,
		MovementHandler,
		NetNodeManager,
		NetworkedNode,
		PathHelper,
		PersistanceHandler,
		ServerHandler,
		ServerLoader,
		SetupHelpers,
		StringifyHelper,
		TypeHelper,
		WorldController,
		WorldHandler,
		ObjectEventHandler,
	]

	var cck_markers: Array[CCKMarker] = get_cck_markers()

	var event_handler: ObjectEventHandler = ObjectEventHandler.new()
	state.state["event_handler"] = event_handler

	var nodes: Array[Node] = get_node_and_children_recursive(root)
	for node: Node in nodes:
		if is_instance_of(node, AnimationMixer):
			if !is_animation_good(node as AnimationMixer):
				print("removing node with bad animation tracks %s" % root.get_path_to(node))
				node.get_parent().remove_child(node)
				node.queue_free()
			else:
				if node is AnimationTree:
					clean_anim_tree(node as AnimationTree)

		for property: Dictionary in node.get_property_list():
			if property["usage"] & PropertyUsageFlags.PROPERTY_USAGE_CATEGORY:
				continue
			if property["usage"] & PropertyUsageFlags.PROPERTY_USAGE_GROUP:
				continue
			if property["usage"] & PropertyUsageFlags.PROPERTY_USAGE_SUBGROUP:
				continue

			# todo: check if any nodes take paths that arnt NodePaths
			@warning_ignore("unsafe_cast")
			if property["type"] == Variant.Type.TYPE_NODE_PATH:
				@warning_ignore("unsafe_cast")
				if !is_path_good(node, root, node[property["name"]] as NodePath):
					print(
						"got invalid path %s in %s.%s, removing."
						% [node[property["name"]], root.get_path_to(node), property["name"]]
					)
					node[property["name"]] = ""

		for group: StringName in node.get_groups():
			if !group.begins_with("cck_"):
				print("removed group %s from node %s" % [group, root.get_path_to(node)])
				node.remove_from_group(group)
		@warning_ignore("untyped_declaration")
		if blacklisted_nodes.any(
			func(blacklist_item) -> bool:
				return is_instance_of(node, blacklist_item),
		):
			node.queue_free()
			continue

		if node is Camera3D:
			(node as Camera3D).clear_current()

		for marker: CCKMarker in cck_markers:
			if !marker.is_allowed_on(type):
				continue

			if marker.supports_multiple_copies():
				for meta_item: StringName in node.get_meta_list():
					if meta_item.split("_")[0] == marker.get_name():
						var values: Dictionary[String, Variant] = { }
						@warning_ignore("unsafe_cast")
						values.assign(node.get_meta(meta_item) as Dictionary)
						values = marker.perform_migrations(values)
						marker.setup(values, node, state)
			else:
				if node.has_meta(marker.get_name()):
					var values: Dictionary[String, Variant] = { }
					@warning_ignore("unsafe_cast")
					values.assign(node.get_meta(marker.get_name()) as Dictionary)
					values = marker.perform_migrations(values)
					marker.setup(values, node, state)

	root.add_child(event_handler)


static func is_path_good(node: Node, root: Node, path: NodePath) -> bool:
	var path_string: String = path
	if path_string.begins_with("/"):
		# absolute paths can never be valid because the location of root is not known
		return false

	if path_string == ".":
		return true

	if path_string.contains(":"):
		# todo: proper handling for properties in paths might be needed eventually
		return false

	var current_node: Node = node
	var path_segments: PackedStringArray = path_string.split("/", false)
	path_segments.reverse()

	for segment: String in path_segments:
		if segment == "..":
			if current_node == root:
				return false
			current_node = current_node.get_parent()
		else:
			current_node = current_node.get_node(segment)
	return true


static func is_animation_good(node: AnimationMixer) -> bool:
	for animation_name: String in node.get_animation_list():
		var animation: Animation = node.get_animation(animation_name)
		for track: int in animation.get_track_count():
			var type: int = animation.track_get_type(track)
			match type:
				Animation.TrackType.TYPE_BEZIER:
					# bezier can modify arbitary float properties
					return false
				Animation.TrackType.TYPE_METHOD:
					return false
				Animation.TrackType.TYPE_VALUE:
					return false
	return true


static func clean_anim_tree(node: AnimationTree) -> void:
	pass


static func setup_world(root: Node) -> WorldController:
	var state: SetupState = SetupState.new()

	setup_object(root, TypeHelper.ObjectType.world, state)

	var world: WorldController
	if state.state.has("spawnpoint"):
		@warning_ignore("unsafe_cast")
		world = WorldController.setup(state.state["spawnpoint"] as Node3D)
	else:
		var spawn_node: Node3D = Node3D.new()
		root.add_child(spawn_node)
		world = WorldController.setup(spawn_node)
	world.add_child(root)

	return world


static func setup_avatar(root: Node, player: Player) -> void:
	var blacklisted_nodes: Array = [
		WorldEnvironment,
		ShaderGlobalsOverride,
		CanvasLayer,
		CanvasItem,
	]

	var state: SetupState = SetupState.new()
	var combined_aabb: AABB = AABB(Vector3.ZERO, Vector3.ZERO)

	state.state["player"] = player

	setup_object(root, TypeHelper.ObjectType.avatar, state)

	var nodes: Array[Node] = get_node_and_children_recursive(root)
	for node: Node in nodes:
		@warning_ignore("untyped_declaration")
		if blacklisted_nodes.any(
			func(blacklist_item: Variant) -> bool:
				return is_instance_of(node, blacklist_item),
		):
			node.queue_free()
			continue

		if node is VisualInstance3D:
			if combined_aabb == AABB(Vector3.ZERO, Vector3.ZERO):
				combined_aabb = (node as VisualInstance3D).get_aabb()
			else:
				var aabb: AABB = (node as VisualInstance3D).get_aabb()
				combined_aabb = combined_aabb.merge(aabb)

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
		(player.collider.shape as CapsuleShape3D).radius = maxf(
			combined_aabb.size.x,
			combined_aabb.size.z,
		)
	else:
		(player.collider.shape as CapsuleShape3D).radius = maxf(
			combined_aabb.size.x,
			combined_aabb.size.z,
		)
		(player.collider.shape as CapsuleShape3D).height = combined_aabb.size.y

	if state.state.has("collider_values"):
		var radius: float = state.state["collider_values"]["radius"]
		var height: float = state.state["collider_values"]["height"]

		radius = maxf(
			(player.collider.shape as CapsuleShape3D).radius / 10,
			minf((player.collider.shape as CapsuleShape3D).radius * 2, radius),
		)

		height = maxf(
			(player.collider.shape as CapsuleShape3D).height / 10,
			minf((player.collider.shape as CapsuleShape3D).height * 2, height),
		)

		if radius > height / 2:
			(player.collider.shape as CapsuleShape3D).height = height
			(player.collider.shape as CapsuleShape3D).radius = radius
		else:
			(player.collider.shape as CapsuleShape3D).radius = radius
			(player.collider.shape as CapsuleShape3D).height = height

	player.collider.position = state.state["collider_values"]["position"]
	player.collider.position.y += (player.collider.shape as CapsuleShape3D).height / 2

	player.position.y += (player.collider.shape as CapsuleShape3D).height / 2
