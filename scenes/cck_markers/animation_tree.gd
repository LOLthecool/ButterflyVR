extends CCKMarker
class_name CCKAnimationTree


func setup(
	values: Dictionary[String, Variant],
	target: Node,
	_state: SetupHelpers.SetupState,
) -> void:
	var player: AnimationPlayer = AnimationPlayer.new()

	@warning_ignore("unsafe_cast")
	for library_name: String in (values["libraries"] as Dictionary).keys():
		var library: Dictionary[String, Animation] = { }
		@warning_ignore("unsafe_cast")
		library.assign(values["libraries"][library_name] as Dictionary)
		var animation_library: AnimationLibrary = AnimationLibrary.new()

		for animation_name: String in library.keys():
			if library[animation_name] is not Animation:
				push_error("invalid animation in CCKAnimationPlayer: %s" % get_name())
				return
			var animation: Animation = library[animation_name]
			animation_library.add_animation(animation_name.split("/", false, 1)[1], animation)

		player.add_animation_library(library_name, animation_library)

	for animation_name: String in player.get_animation_list():
		var animation: Animation = player.get_animation(animation_name)
		for i: int in range(0, animation.get_track_count()):
			if animation.track_get_type(i) == Animation.TrackType.TYPE_METHOD \
					or animation.track_get_type(i) == Animation.TrackType.TYPE_VALUE \
					or animation.track_get_type(i) == Animation.TrackType.TYPE_ANIMATION:
				player.free()
				push_error(
					"unsafe animation %s in node %s. ignoring this animation player"
					% [animation_name, values["player_name"]]
				)
				return

	target.add_child(player)

	@warning_ignore("unsafe_cast")
	player.root_node = player.get_path_to(target.get_node(values["root_node"] as String))

	var tree: AnimationTree = AnimationTree.new()

	tree.active = values["active"]

	var tree_data: Dictionary[String, Variant] = { }
	@warning_ignore("unsafe_cast")
	tree_data.assign(values["tree_data"] as Dictionary)
	tree.tree_root = generate_tree(tree_data)

	var parameters: Dictionary[String, Variant] = values["parameters"]
	for property_name: String in parameters.keys():
		tree.set("parameters/" + property_name, parameters[property_name])

	tree.name = values["name"]
	target.add_child(tree, true)

	tree.anim_player = tree.get_path_to(player)


func generate_tree(data: Dictionary[String, Variant]) -> AnimationRootNode:
	return generate_anim_node_recursive(data) as AnimationRootNode


func generate_anim_node_recursive(values: Dictionary[String, Variant]) -> AnimationNode:
	var result: AnimationNode

	match values["type"]:
		"AnimationNodeAnimation":
			var node: AnimationNodeAnimation = AnimationNodeAnimation.new()
			node.advance_on_start = values["advance_on_start"]
			node.animation = values["animation"]
			node.loop_mode = values["loop_mode"]
			node.play_mode = values["play_mode"]
			node.start_offset = values["start_offset"]
			node.stretch_time_scale = values["stretch_time_scale"]
			node.timeline_length = values["timeline_length"]
			node.use_custom_timeline = values["use_custom_timeline"]
			result = node

		"AnimationNodeBlendSpace1D":
			var node: AnimationNodeBlendSpace1D = AnimationNodeBlendSpace1D.new()
			node.blend_mode = values["blend_mode"]
			node.max_space = values["max_space"]
			node.min_space = values["min_space"]
			node.snap = values["snap"]
			node.sync = values["sync"]
			node.value_label = values["value_label"]

			var blend_point_node_data: Array[Dictionary] = values["blend_point_nodes"]
			var blend_point_positions: Array[float] = values["blend_point_positions"]

			for index: int in range(0, blend_point_node_data.size()):
				var data: Dictionary[String, Variant] = { }
				data.assign(blend_point_node_data[index])

				var sub_node: AnimationRootNode = generate_anim_node_recursive(data) as AnimationRootNode
				var position: float = blend_point_positions[index]

				node.add_blend_point(sub_node, position)
			result = node

		"AnimationNodeBlendSpace2D":
			var node: AnimationNodeBlendSpace2D = AnimationNodeBlendSpace2D.new()
			node.auto_triangles = false
			node.blend_mode = values["blend_mode"]
			node.max_space = values["max_space"]
			node.min_space = values["min_space"]
			node.snap = values["snap"]
			node.sync = values["sync"]
			node.x_label = values["x_label"]
			node.y_label = values["y_label"]

			var blend_point_node_data: Array[Dictionary] = values["blend_point_nodes"]
			var blend_point_positions: Array[Vector2] = values["blend_point_positions"]
			var triangle_point_indexes: Array[Array] = values["triangles_point_indexes"]

			for index: int in range(0, blend_point_node_data.size()):
				var data: Dictionary[String, Variant] = { }
				data.assign(blend_point_node_data[index])

				var sub_node: AnimationRootNode = generate_anim_node_recursive(data) as AnimationRootNode
				var position: Vector2 = blend_point_positions[index]

				node.add_blend_point(sub_node, position)

			for triangle: Array in triangle_point_indexes:
				@warning_ignore("unsafe_cast")
				node.add_triangle(triangle[0] as int, triangle[1] as int, triangle[2] as int)

			node.auto_triangles = values["auto_triangles"]
			result = node

		"AnimationNodeBlendTree":
			var node: AnimationNodeBlendTree = AnimationNodeBlendTree.new()
			var nodes: Dictionary[String, Dictionary] = values["nodes"]
			var positions: Dictionary[String, Vector2] = values["positions"]
			for node_name: String in nodes.keys():
				var data: Dictionary[String, Variant] = { }
				data.assign(nodes[node_name])

				if data["type"] == "AnimationNodeOutput":
					node.set_node_position(node_name, positions[node_name])
				else:
					var sub_node: AnimationNode = generate_anim_node_recursive(data)
					node.add_node(node_name, sub_node, positions[node_name])
			result = node

		"AnimationNodeStateMachine":
			var node: AnimationNodeStateMachine = AnimationNodeStateMachine.new()
			node.allow_transition_to_self = values["allow_transition_to_self"]
			node.reset_ends = values["reset_ends"]
			node.state_machine_type = values["state_machine_type"]

			var nodes: Dictionary[String, Dictionary] = values["nodes"]
			var node_positions: Dictionary[String, Vector2] = values["node_positions"]
			var transitions: Array[Dictionary] = values["transitions"]

			for node_name: String in nodes.keys():
				var data: Dictionary[String, Variant] = { }
				data.assign(nodes[node_name])

				# special case handling for start and end nodes
				if data["type"] == "AnimationNodeStartState" \
						or data["type"] == "AnimationNodeEndState":
					node.set_node_position(node_name, node_positions[node_name])
				else:
					var sub_node: AnimationRootNode = \
							generate_anim_node_recursive(data) as AnimationRootNode
					node.add_node(node_name, sub_node, node_positions[node_name])

			for transition_data: Dictionary in transitions:
				var data: Dictionary[String, Variant] = { }
				data.assign(transition_data)

				var transition: AnimationNodeStateMachineTransition = \
						AnimationNodeStateMachineTransition.new()

				transition.break_loop_at_end = data["break_loop_at_end"]
				transition.priority = data["priority"]
				transition.reset = data["reset"]
				transition.switch_mode = data["switch_mode"]

				var curve_data: Dictionary[String, Variant] = { }
				@warning_ignore("unsafe_cast")
				curve_data.assign(data["xfade_curve"] as Dictionary)
				transition.xfade_curve = generate_curve(curve_data)
				transition.xfade_time = data["xfade_time"]

				if data["auto_transition"]:
					transition.advance_mode = \
							AnimationNodeStateMachineTransition.ADVANCE_MODE_AUTO
				else:
					transition.advance_mode = \
							AnimationNodeStateMachineTransition.ADVANCE_MODE_ENABLED

				@warning_ignore("unsafe_cast")
				node.add_transition(data["source"] as String, data["target"] as String, transition)
			result = node

		"AnimationNodeTimeSeek":
			var node: AnimationNodeTimeSeek = AnimationNodeTimeSeek.new()
			node.explicit_elapse = values["explicit_elapse"]
			result = node

		"AnimationNodeOneShot":
			var node: AnimationNodeOneShot = AnimationNodeOneShot.new()
			node.abort_on_reset = values["abort_on_reset"]
			node.autorestart = values["autorestart"]
			node.autorestart_delay = values["autorestart_delay"]
			node.autorestart_random_delay = values["autorestart_random_delay"]
			node.break_loop_at_end = values["break_loop_at_end"]
			node.fadein_time = values["fadein_time"]
			node.fadeout_time = values["fadeout_time"]
			node.mix_mode = values["mix_mode"]

			var curve_data: Dictionary[String, Variant] = { }
			@warning_ignore("unsafe_cast")
			curve_data.assign(values["fadein_curve"] as Dictionary)
			node.fadein_curve = generate_curve(curve_data)

			@warning_ignore("unsafe_cast")
			curve_data.assign(values["fadeout_curve"] as Dictionary)
			node.fadeout_curve = generate_curve(curve_data)

			result = node

		"AnimationNodeTransition":
			var node: AnimationNodeTransition = AnimationNodeTransition.new()
			node.allow_transition_to_self = values["allow_transition_to_self"]
			node.input_count = values["input_count"]
			node.xfade_time = values["xfade_time"]
			var curve_data: Dictionary[String, Variant] = { }
			@warning_ignore("unsafe_cast")
			curve_data.assign(values["xfade_curve"] as Dictionary)
			node.xfade_curve = generate_curve(curve_data)

			# todo: some extra info for the inputs is in here
			# but we would need to add after inputs are added
			result = node

		"AnimationNodeTimeScale":
			result = AnimationNodeTimeScale.new()
		"AnimationNodeAdd2":
			result = AnimationNodeAdd2.new()
		"AnimationNodeAdd3":
			result = AnimationNodeAdd3.new()
		"AnimationNodeBlend2":
			result = AnimationNodeBlend2.new()
		"AnimationNodeBlend3":
			result = AnimationNodeBlend3.new()
		"AnimationNodeSub2":
			result = AnimationNodeSub2.new()

		_:
			push_error("got invalid AnimationNode data with type '%s'" % values["type"])
			result = AnimationRootNode.new()

	if values.has("inputs"):
		var inputs: Array[String] = values["inputs"]
		for input: String in inputs:
			result.add_input(input)

	return result


func generate_curve(values: Dictionary[String, Variant]) -> Curve:
	if values.is_empty():
		return null

	var curve: Curve = Curve.new()
	curve.clear_points()

	curve.bake_resolution = values["bake_resolution"]
	curve.max_domain = values["max_domain"]
	curve.max_value = values["max_value"]
	curve.min_domain = values["min_domain"]
	curve.min_value = values["min_value"]

	var points: Array[Dictionary] = values["points"]
	for point_data: Dictionary in points:
		var point: Dictionary[String, Variant] = { }
		point.assign(point_data)

		@warning_ignore("unsafe_cast")
		curve.add_point(
			point["position"] as Vector2,
			point["left_tangent"] as float,
			point["right_tangent"] as float,
			point["left_mode"] as int,
			point["right_mode"] as int,
		)

	curve.bake()

	return curve


func get_name() -> String:
	return "CCKAnimationTree"


func perform_migrations(values: Dictionary[String, Variant]) -> Dictionary[String, Variant]:
	var current_version: String = get_current_version_string()
	match values["version"]:
		current_version:
			return values
		_:
			@warning_ignore("unsafe_cast")
			push_error(
				"had no valid migration for %s version %s. content may be broken" \
						% [get_name(), values["version"] as String]
			)
			return values


func get_current_version_string() -> String:
	return "1"


func supports_multiple_copies() -> bool:
	return true


func is_allowed_on(_object_type: TypeHelper.ObjectType) -> bool:
	return true
