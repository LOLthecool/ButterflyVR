extends CCKMarker
class_name ShaderMarker


func setup(
	values: Dictionary[String, Variant],
	target: Node,
	_state: SetupHelpers.SetupState,
	root: Node,
) -> void:
	var material:ShaderMaterial = values["material"]
	
	var code:String = values["shader_code"]
	if !code.begins_with("#pragma disable_preprocessor\n") or code.substr(1).contains("#"):
		push_warning("rejecting shader (contained preprocessing instructions)")
		return
	
	var shader:Shader = Shader.new()
	shader.code = code
	material.shader = shader
	
	if !target.get_property_list().any(func(x:Dictionary) -> bool:
		return x["name"] == values["target_property"] and x["class_name"] == "ShaderMaterial"):
			push_warning("couldnt find target property for ShaderMarker")
			return
	target[values["target_property"]] = material


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
	return "ShaderMarker"


func supports_multiple_copies() -> bool:
	return true


func is_allowed_on(_object_type: TypeHelper.ObjectType) -> bool:
	return true
