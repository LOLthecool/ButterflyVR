extends CCKMarker
class_name EnableActionMarker

func setup(values:Dictionary[String, Variant], target:Node, state:SetupHelpers.SetupState) -> void:
	if values["action_version"] != "1":
		return
	var handler:ObjectEventHandler = state.state["event_handler"]
	@warning_ignore("unsafe_cast")
	handler.register_action(ObjectEventHandler.EnableAction.create(
			values["action_id"] as PackedByteArray, values["take_parameter"] as bool, 
			target, values["target_property"] as int, values["active"] as bool, 
			values["custom_parameters"] as Array))

func get_name() -> String:
	return "EnableAction"

func supports_multiple_copies() -> bool:
	return true

func is_allowed_on(_object_type: LRUCache.ObjectType) -> bool:
	return true
