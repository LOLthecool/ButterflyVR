extends CCKMarker
class_name CCKAnimationPlayer

func setup(values:Dictionary[String, Variant], target:Node, _state:SetupHelpers.SetupState) -> void:
	var player:AnimationPlayer = AnimationPlayer.new()
	
	@warning_ignore("unsafe_cast")
	for library_name:String in (values["libraries"] as Dictionary).keys():
		var library:Dictionary[String, Animation] = {}
		@warning_ignore("unsafe_cast")
		library.assign(values["libraries"][library_name] as Dictionary)
		var animation_library:AnimationLibrary = AnimationLibrary.new()
		
		for animation_name:String in library.keys():
			if library[animation_name] is not Animation:
				push_error("invalid animation in CCKAnimationPlayer: %s" % get_name())
				return
			var animation:Animation = library[animation_name]
			animation_library.add_animation(animation_name.split("/", false, 1)[1], animation)
		
		player.add_animation_library(library_name, animation_library)
	
	player.name = values["player_name"]
	target.add_child(player, true)
	
	player.active = values["active"]
	player.speed_scale = values["playback_speed"]
	@warning_ignore("unsafe_cast")
	player.root_node = player.get_path_to(target.get_node(values["root_node"] as String))
	if player.active:
		@warning_ignore("unsafe_cast")
		player.play(values["animation"] as String, -1, values["playback_speed"] as float)

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

func get_name() -> String:
	return "CCKAnimationPlayer"

func supports_multiple_copies() -> bool:
	return true

func is_allowed_on(_object_type: TypeHelper.ObjectType) -> bool:
	return true
