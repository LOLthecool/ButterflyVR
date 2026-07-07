extends CCKMarker
class_name CCKAnimationPlayer

func setup(values:Dictionary[String, Variant], target:Node, _state:SetupHelpers.SetupState) -> void:
	if values["marker_version"] != "1" or values["animation"] is not Animation:
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
	return "CCKAnimationPlayer"

func supports_multiple_copies() -> bool:
	return true

func is_allowed_on(_object_type: LRUCache.ObjectType) -> bool:
	return true
