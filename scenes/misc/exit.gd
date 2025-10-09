extends Button

func _pressed() -> void:
	# todo: should probably tell api when we exit so it can update status, expire temp tokens, etc
	var tree:SceneTree = get_tree()
	get_parent().remove_child(self) # to avoid freeing ourself before finishing
	GlobalWorldAccess.current_world.queue_free()
	await tree.physics_frame
	# disconnect first, if connected, so the server isnt waiting for timeout
	(NetworkManager as NetNodeManager).stop()
	get_tree().free() # probably a better way to do this but it seems to work fine
