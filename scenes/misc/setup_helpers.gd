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


# IMPORTANT: this must be called BEFORE the node is added to the scene tree, failing to do so
# may allow path traversal attacks when marker setup is performed.
static func setup_object(root: Node, type: TypeHelper.ObjectType, state: SetupState) -> void:
	var cck_markers: Array[CCKMarker] = get_cck_markers()

	var event_handler: ObjectEventHandler = ObjectEventHandler.new()
	state.state["event_handler"] = event_handler

	var nodes: Array[Node] = get_node_and_children_recursive(root)
	for node: Node in nodes:
		if is_instance_of(node, AnimationMixer):
			if !is_animation_good(node as AnimationMixer):
				print("removing node with bad animation tracks %s" % node)
				node.get_parent().remove_child(node)
				node.queue_free()
			else:
				if node is AnimationTree:
					clean_anim_tree((node as AnimationTree).tree_root)

		clean_paths(node, root, node)

		for group: StringName in node.get_groups():
			if !group.begins_with("cck_"):
				print("removed group %s from node %s" % [group, node])
				node.remove_from_group(group)
		@warning_ignore("untyped_declaration")
		if !NodeWhitelist.whitelisted_nodes.any(
			func(whitelist_item) -> bool:
				# whitelisted_item is type GDScriptNativeClass
				return is_instance_of(node, whitelist_item),
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
						marker.setup(values, node, state, root)
			else:
				if node.has_meta(marker.get_name()):
					var values: Dictionary[String, Variant] = { }
					@warning_ignore("unsafe_cast")
					values.assign(node.get_meta(marker.get_name()) as Dictionary)
					values = marker.perform_migrations(values)
					marker.setup(values, node, state, root)

	root.add_child(event_handler)


static func clean_paths(object: Object, root: Node, original_node: Node) -> void:
	for property: Dictionary in object.get_property_list():
		if property["usage"] & PropertyUsageFlags.PROPERTY_USAGE_CATEGORY:
			continue
		if property["usage"] & PropertyUsageFlags.PROPERTY_USAGE_GROUP:
			continue
		if property["usage"] & PropertyUsageFlags.PROPERTY_USAGE_SUBGROUP:
			continue

		# workaround for a bug i dont wanna deal with right now
		if property["name"] == "states/End/node" and object is AnimationNodeStateMachine:
			continue
		if property["name"] == "states/Start/node" and object is AnimationNodeStateMachine:
			continue

		if (
			property["type"] == Variant.Type.TYPE_NODE_PATH
			or property["type"] == Variant.Type.TYPE_OBJECT
			or property["type"] == Variant.Type.TYPE_ARRAY
			or property["type"] == Variant.Type.TYPE_DICTIONARY
		):
			if object[property["name"]]:
				@warning_ignore("unsafe_cast")
				clean_property(
					object[property["name"]],
					object,
					original_node,
					root,
					property["name"] as String,
				)


static func clean_property(
	property: Variant,
	object: Object,
	original_node: Node,
	root: Node,
	property_name: String,
) -> void:
	# todo: check if any nodes take paths that arnt NodePaths
	if property is NodePath:
		if is_instance_of(object, Node):
			@warning_ignore("unsafe_cast")
			if !PathHelper.is_path_good(object as Node, root, property as NodePath):
				print(
					"got invalid path %s in %s.%s.%s, freeing object."
					% [property, original_node, object, property_name]
				)
				object.free()
		else:
			@warning_ignore("unsafe_cast")
			if !PathHelper.is_path_good(original_node, root, property as NodePath):
				print(
					"got invalid path %s in %s.%s, freeing object."
					% [property, original_node, object]
				)
				original_node.free()

	elif is_instance_of(property, Object):
		if property:
			@warning_ignore("unsafe_cast")
			clean_paths(property as Object, root, original_node)

	elif property is Array:
		@warning_ignore("unsafe_cast")
		for sub: Variant in property as Array:
			clean_property(sub, object, original_node, root, property_name)

	elif property is Dictionary:
		@warning_ignore("unsafe_cast")
		for sub: Variant in (property as Dictionary).keys():
			clean_property(sub, object, original_node, root, property_name)

		@warning_ignore("unsafe_cast")
		for sub: Variant in (property as Dictionary).values():
			clean_property(sub, object, original_node, root, property_name)


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


static func clean_anim_tree(untyped_node: AnimationRootNode) -> void:
	if untyped_node is AnimationNodeBlendSpace1D:
		var node: AnimationNodeBlendSpace1D = untyped_node
		for index: int in range(0, node.get_blend_point_count()):
			clean_anim_tree(node.get_blend_point_node(index))

	elif untyped_node is AnimationNodeBlendSpace2D:
		var node: AnimationNodeBlendSpace2D = untyped_node
		for index: int in range(0, node.get_blend_point_count()):
			clean_anim_tree(node.get_blend_point_node(index))

	elif untyped_node is AnimationNodeBlendTree:
		var node: AnimationNodeBlendTree = untyped_node
		for node_name: String in node.get_node_list():
			var sub_node: AnimationNode = node.get_node(node_name)
			if sub_node is AnimationRootNode:
				clean_anim_tree(sub_node as AnimationRootNode)

	elif untyped_node is AnimationNodeStateMachine:
		var node: AnimationNodeStateMachine = untyped_node
		for node_name: String in node.get_node_list():
			var sub_node: AnimationNode = node.get_node(node_name) as AnimationNode
			if sub_node is AnimationRootNode:
				clean_anim_tree(sub_node as AnimationRootNode)
		for index: int in range(0, node.get_transition_count()):
			if node.get_transition(index).advance_expression != "":
				print(
					"removing advance expression '%s' on transition with index %s inside anim tree %s"
					% [node.get_transition(index).advance_expression, index, node]
				)
				node.get_transition(index).advance_expression = ""


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
