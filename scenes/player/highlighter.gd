extends RayCast3D

var current_target: Highlighter


func _physics_process(_delta: float) -> void:
	if !is_colliding() and current_target:
		current_target.end_highlight()
		current_target = null
	elif get_collider() != current_target:
		if current_target:
			current_target.end_highlight()

		if get_collider() and get_collider().has_meta("highlighter"):
			var highlighter: Highlighter = get_collider().get_meta("highlighter")
			current_target = highlighter
			current_target.start_highlight()
