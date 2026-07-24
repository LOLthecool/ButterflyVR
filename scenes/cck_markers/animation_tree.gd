extends CCKMarker
class_name CCKAnimationTree

func setup(values:Dictionary[String, Variant], target:Node, _state:SetupHelpers.SetupState) -> void:
	pass

func get_name() -> String:
	return "CCKAnimationTree"

func perform_migrations(values:Dictionary[String, Variant]) -> Dictionary[String, Variant]:
	var current_version:String = get_current_version_string()
	match values["version"]:
		current_version:
			return values
		_:
			@warning_ignore("unsafe_cast")
			push_error("had no valid migration for %s version %s. content may be broken" \
					% [get_name(), values["version"] as String])
			return values

func get_current_version_string() -> String:
	return "1"

func supports_multiple_copies() -> bool:
	return true

func is_allowed_on(_object_type: TypeHelper.ObjectType) -> bool:
	return true
