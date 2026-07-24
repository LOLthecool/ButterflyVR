extends CCKMarker
class_name Grabbable

func setup(values:Dictionary[String, Variant], target:Node, _state:SetupHelpers.SetupState) -> void:
	@warning_ignore("unsafe_cast")
	var hitbox:CollisionObject3D = target.get_node(values["hitbox"] as String)
	hitbox.collision_layer = hitbox.collision_layer | 2
	
	var collider_values:Dictionary[String, Variant] = {}
	
	collider_values["max_grab_distance"] = values["max_grab_distance"]
	collider_values["target"] = target
	
	if values.has("snap_offset_position") and values.has("snap_offset_rotation"):
		collider_values["snap_offset_position"] = values["snap_offset_position"]
		collider_values["snap_offset_rotation"] = values["snap_offset_rotation"]
	
	@warning_ignore("unsafe_cast")
	if values.has("highlight_mesh"):
		@warning_ignore("unsafe_cast")
		var highlight_mesh:MeshInstance3D = hitbox.get_node(values["highlight_mesh"] as String)
		var new_node:Highlighter = Highlighter.new()
		highlight_mesh.add_child(new_node)
		new_node.geometry = highlight_mesh
		new_node.setup()
		hitbox.set_meta("highlighter", new_node)
	
	hitbox.set_meta("grabbable_info", collider_values)

func perform_migrations(values:Dictionary[String, Variant]) -> Dictionary[String, Variant]:
	var current_version:String = get_current_version_string()
	match values["version"]:
		current_version:
			return values
		"1":
			if values["highlight_mesh"] == "":
				values.erase("highlight_mesh")
			values.erase("snap_on_grab")
			values["version"] = "2"
			return perform_migrations(values)
		_:
			push_error("had no valid migration for %s version %s. content may be broken" \
			% [get_name(), values["version"]])
			return values

func get_current_version_string() -> String:
	return "2"

func get_name() -> String:
	return "Grabbable"

func supports_multiple_copies() -> bool:
	return false

func is_allowed_on(_object_type: LRUCache.ObjectType) -> bool:
	return true
