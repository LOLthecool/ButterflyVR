extends CCKMarker
class_name DisplayActionMarker

func setup(values:Dictionary[String, Variant], target:Node, state:SetupHelpers.SetupState) -> void:
	if values["action_version"] != "1" or target is not Label3D:
		return
	var handler:ObjectEventHandler = state.state["event_handler"]
	@warning_ignore("unsafe_cast")
	handler.register_action(ObjectEventHandler.DisplayAction.create(
			values["action_id"] as PackedByteArray, values["text"] as String, 
			target as Label3D, values["active"] as bool, 
			values["custom_parameters"] as Array))

func get_name() -> String:
	return "DisplayAction"

func supports_multiple_copies() -> bool:
	return true

func is_allowed_on(_object_type: LRUCache.ObjectType) -> bool:
	return true
