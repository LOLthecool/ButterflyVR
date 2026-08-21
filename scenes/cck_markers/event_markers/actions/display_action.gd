extends CCKMarker
class_name DisplayActionMarker


func setup(
	values: Dictionary[String, Variant],
	target: Node,
	state: SetupHelpers.SetupState,
) -> void:
	if target is not Label3D:
		return
	var handler: ObjectEventHandler = state.state["event_handler"]
	@warning_ignore("unsafe_cast")
	handler.register_action(ObjectEventHandler.DisplayAction.create(
			values["action_id"] as PackedByteArray,
			values["text"] as String,
			target as Label3D,
			values["active"] as bool,
			values["custom_parameters"] as Array,
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
	return "DisplayAction"


func supports_multiple_copies() -> bool:
	return true


func is_allowed_on(_object_type: TypeHelper.ObjectType) -> bool:
	return true
