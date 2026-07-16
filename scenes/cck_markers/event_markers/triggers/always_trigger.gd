extends CCKMarker
class_name AlwaysTriggerMarker

func setup(values:Dictionary[String, Variant], _target:Node, state:SetupHelpers.SetupState) -> void:
	if values["trigger_version"] != "1":
		return
	var handler:ObjectEventHandler = state.state["event_handler"]
	@warning_ignore("unsafe_cast")
	handler.register_trigger(ObjectEventHandler.AlwaysTrigger.create(
			values["include_tick_count"] as bool, values["active"] as bool, 
			values["custom_parameters"] as Array, values["targets"] as Array[PackedByteArray]))

func get_name() -> String:
	return "AlwaysTrigger"

func supports_multiple_copies() -> bool:
	return true

func is_allowed_on(_object_type: LRUCache.ObjectType) -> bool:
	return true
