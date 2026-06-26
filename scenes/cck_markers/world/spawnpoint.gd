extends CCKMarker
class_name Spawnpoint

func setup(values:Dictionary[String, Variant], target:Node, state:SetupHelpers.SetupState) -> void:
	if values["marker_version"] == "1" and target is Node3D:
		state.state["spawnpoint"] = target

func get_name() -> String:
	return "Spawnpoint"

func supports_multiple_copies() -> bool:
	return false

func is_allowed_on(object_type: LRUCache.ObjectType) -> bool:
	if object_type == LRUCache.ObjectType.world:
		return true
	return false
