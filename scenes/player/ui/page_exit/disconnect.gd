extends Button

func _pressed() -> void:
	await GlobalWorldHandler.disconnect_from_world(false)
