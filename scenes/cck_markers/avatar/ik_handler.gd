extends CCKMarker
class_name IKMarker

func setup(values:Dictionary[String, Variant], target:Node, state:SetupHelpers.SetupState) -> void:
	return

func get_name() -> String:
	return "IKMarker"

func supports_multiple_copies() -> bool:
	return false

func is_allowed_on(object_type: LRUCache.ObjectType) -> bool:
	if object_type == LRUCache.ObjectType.avatar:
		return true
	return false
