extends CCKMarker
class_name Grabbable

func setup(values:Dictionary[String, Variant], target:Node, _state:SetupHelpers.SetupState) -> void:
	if values["marker_version"] != "1":
		return
	@warning_ignore("unsafe_cast")
	var hitbox:Area3D = target.get_child(values["hitbox"] as int)
	hitbox.collision_layer = hitbox.collision_layer | 2
	
	var collider_values:Dictionary[String, Variant] = {
			"max_grab_distance":values["max_grab_distance"],
			"snap_on_grab":values["snap_on_grab"]}
	
	if values["snap_on_grab"]:
		collider_values["snap_offset_position"] = values["snap_offset_position"]
		collider_values["snap_offset_rotation"] = values["snap_offset_rotation"]
	
	hitbox.set_meta("grabbable_info", values)
	
	@warning_ignore("unsafe_cast")
	if values["highlight_mesh"] as String != "":
		@warning_ignore("unsafe_cast")
		var highlight_mesh:MeshInstance3D = hitbox.get_node(values["highlight_mesh"] as String)
		hitbox.set_script(Highlighter)
		hitbox.set("geometry", highlight_mesh)
		hitbox.call("setup")

func get_name() -> String:
	return "Grabbable"

func supports_multiple_copies() -> bool:
	return false

func is_allowed_on(_object_type: LRUCache.ObjectType) -> bool:
	return true
