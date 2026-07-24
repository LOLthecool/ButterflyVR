extends CCKMarker
class_name Spawnpoint

func setup(values:Dictionary[String, Variant], target:Node, state:SetupHelpers.SetupState) -> void:
	# todo: dynamic adding and removal of spawnpoints
	# todo: multiple spawnpoint support
	if target is Node3D and values["enabled"]:
		state.state["spawnpoint"] = target

func perform_migrations(values:Dictionary[String, Variant]) -> Dictionary[String, Variant]:
	var current_version:String = get_current_version_string()
	match values["version"]:
		current_version:
			return values
		"1":
			values["enabled"] = true
			values["version"] = "2"
			return perform_migrations(values)
		_:
			push_error("had no valid migration for %s version %s. content may be broken" \
			% [get_name(), values["version"]])
			return values

func get_current_version_string() -> String:
	return "2"

func get_name() -> String:
	return "Spawnpoint"

func supports_multiple_copies() -> bool:
	return false

func is_allowed_on(object_type: LRUCache.ObjectType) -> bool:
	if object_type == LRUCache.ObjectType.world:
		return true
	return false
