@abstract
extends RefCounted
class_name CCKMarker


# remember to call PathHelper.is_path_good() if one of your values gets used as a path
# being outside the scene tree during setup is not guarenteed
@abstract
func setup(
	values: Dictionary[String, Variant],
	target: Node,
	state: SetupHelpers.SetupState,
	root: Node,
) -> void


@abstract
func perform_migrations(values: Dictionary[String, Variant]) -> Dictionary[String, Variant]


@abstract
func get_current_version_string() -> String


@abstract
func get_name() -> String


@abstract
func supports_multiple_copies() -> bool


@abstract
func is_allowed_on(object_type: TypeHelper.ObjectType) -> bool
