extends CCKMarker
class_name AlwaysTriggerMarker

func setup(values:Dictionary[String, Variant], _target:Node, state:SetupHelpers.SetupState) -> void:
	var handler:ObjectEventHandler = state.state["event_handler"]
	@warning_ignore("unsafe_cast")
	handler.register_trigger(ObjectEventHandler.AlwaysTrigger.create(
			values["include_tick_count"] as bool, values["active"] as bool, 
			values["custom_parameters"] as Array, values["targets"] as Array[PackedByteArray]))

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
	return "AlwaysTrigger"

func supports_multiple_copies() -> bool:
	return true

func is_allowed_on(_object_type: LRUCache.ObjectType) -> bool:
	return true
