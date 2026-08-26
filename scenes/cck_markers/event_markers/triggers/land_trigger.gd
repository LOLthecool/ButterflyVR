extends CCKMarker
class_name LandTriggerMarker


func setup(
	values: Dictionary[String, Variant],
	_target: Node,
	state: SetupHelpers.SetupState,
	_root: Node,
) -> void:
	var handler: ObjectEventHandler = state.state["event_handler"]
	@warning_ignore("unsafe_cast")
	handler.register_trigger(ObjectEventHandler.LandTrigger.create(
			state.state["player"] as Player,
			values["active"] as bool,
			values["custom_parameters"] as Array,
			values["targets"] as Array[PackedByteArray],
		))


func perform_migrations(values: Dictionary[String, Variant]) -> Dictionary[String, Variant]:
	var current_version: String = get_current_version_string()
	match values["version"]:
		current_version:
			return values
		_:
			@warning_ignore("unsafe_cast")
			push_error(
				"had no valid migration for %s version %s. content may be broken" \
						% [get_name(), values["version"] as String]
			)
			return values


func get_current_version_string() -> String:
	return "1"


func get_name() -> String:
	return "LandTrigger"


func supports_multiple_copies() -> bool:
	return true


func is_allowed_on(object_type: TypeHelper.ObjectType) -> bool:
	return object_type == TypeHelper.ObjectType.avatar
