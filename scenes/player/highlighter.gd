extends RayCast3D

var current_target:Node

func _physics_process(_delta: float) -> void:
	if !is_colliding():
		if current_target and current_target.has_method("end_highlight"):
			@warning_ignore("unsafe_method_access")
			current_target.end_highlight()
			current_target = null
	elif get_collider() != current_target and get_collider().has_method("start_highlight"):
		if current_target and current_target.has_method("end_highlight"):
			@warning_ignore("unsafe_method_access")
			current_target.end_highlight()
		current_target = get_collider()
		@warning_ignore("unsafe_method_access")
		current_target.start_highlight()
