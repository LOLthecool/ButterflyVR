extends CCKMarker
class_name AvatarColliderConfig

func setup(values:Dictionary[String, Variant], _target:Node, state:SetupHelpers.SetupState) -> void:
	state.state["collider_values"] = {
		"radius":values["radius"],
		"height":values["height"]
	}

func perform_migrations(values:Dictionary[String, Variant]) -> Dictionary[String, Variant]:
	var current_version:String = get_current_version_string()
	match values["version"]:
		current_version:
			return values
		_:
			push_error("had no valid migration for %s version %s. content may be broken" \
			% [get_name(), values["version"]])
			return values

func get_current_version_string() -> String:
	return "1"

func get_name() -> String:
	return "AvatarColliderConfig"

func supports_multiple_copies() -> bool:
	return false

func is_allowed_on(object_type: LRUCache.ObjectType) -> bool:
	if object_type == LRUCache.ObjectType.avatar:
		return true
	return false
