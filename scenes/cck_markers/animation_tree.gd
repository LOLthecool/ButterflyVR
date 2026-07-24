extends CCKMarker
class_name CCKAnimationTree

func setup(values:Dictionary[String, Variant], target:Node, _state:SetupHelpers.SetupState) -> void:
	if values["animation"] is not Animation:
		return
	var player:AnimationPlayer = AnimationPlayer.new()
	var library:AnimationLibrary = AnimationLibrary.new()
	
	@warning_ignore("unsafe_cast")
	library.add_animation("InternalCCKAnimation", values["animation"] as Animation)
	player.add_animation_library("InternalCCKAnimationLibrary", library)
	
	target.add_child(player)
	
	player.active = values["active"]
	player.speed_scale = values["playback_speed"]
	@warning_ignore("unsafe_cast")
	player.root_node = player.get_path_to(target.get_node(values["root_node"] as String))
	if player.active:
		@warning_ignore("unsafe_cast")
		player.play("InternalCCKAnimationLibrary/InternalCCKAnimation", -1, values["playback_speed"] as float)

func get_name() -> String:
	return "CCKAnimationTree"

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

func supports_multiple_copies() -> bool:
	return true

func is_allowed_on(_object_type: LRUCache.ObjectType) -> bool:
	return true
