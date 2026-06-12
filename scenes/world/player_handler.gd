extends MessageHandler

func _physics_process(_delta: float) -> void:
	if NetworkManager.is_server():
		for player:PackedByteArray in NetworkManager.get_new_joins():
			send_message_final([player], [_get_value_type(null, 0)])

func _get_value_type(_previous_value: Variant, idx: int) -> TypeHelper.NetworkedValueTypes:
	match idx:
		0:
			return TypeHelper.NetworkedValueTypes.ByteArray
	return TypeHelper.NetworkedValueTypes.End

func _process_message(values: Array) -> void:
	@warning_ignore("unsafe_cast")
	var player_owner:PackedByteArray = PackedByteArray(values[0] as Array)
	var world:WorldController = GlobalWorldHandler.current_world
	var player:Player = preload("res://scenes/player/player.tscn").instantiate()
	player.networker.owner_id = player_owner
	world.add_child(player)
	player.global_transform = world.spawn_point.global_transform
