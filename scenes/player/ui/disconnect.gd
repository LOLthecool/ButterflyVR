extends Button


func _pressed() -> void:
	var tree: SceneTree = get_tree()
	get_parent().remove_child(self) # to avoid freeing ourself before finishing
	GlobalWorldHandler.current_world.queue_free()
	await tree.physics_frame
	# disconnect first, if connected, so the server isnt waiting for timeout
	NetworkManager.stop()
	GlobalAccountHandler.logout()
	tree.change_scene_to_packed(preload("res://scenes/startup/loading.tscn"))
	queue_free()
