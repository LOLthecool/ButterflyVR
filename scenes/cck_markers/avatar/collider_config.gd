extends CCKMarker
class_name AvatarColliderConfig

func setup(values:Dictionary[String, Variant], _target:Node, state:SetupHelpers.SetupState) -> void:
	if values["marker_version"] == "1":
		state.state["collider_values"] = {
			"radius":values["radius"],
			"height":values["height"]
		}

func get_name() -> String:
	return "AvatarColliderConfig"

func supports_multiple_copies() -> bool:
	return false

func is_allowed_on(object_type: LRUCache.ObjectType) -> bool:
	if object_type == LRUCache.ObjectType.avatar:
		return true
	return false
