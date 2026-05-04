extends MessageHandler

func _physics_process(_delta: float) -> void:
	for player:PackedByteArray in NetworkManager.get_new_joins():
		send_message_final([player], [_get_value_type(null, 0)])

func _get_value_type(_previous_value: Variant, idx: int) -> TypeHelper.NetworkedValueTypes:
	match idx:
		0:
			return TypeHelper.NetworkedValueTypes.ByteArray
	return TypeHelper.NetworkedValueTypes.End

func _process_message(values: Array) -> void:
	var player_owner:int = values[0]
	var world:WorldController = GlobalWorldHandler.current_world
	var player:Player = preload("res://scenes/player/player.tscn").instantiate()
	player.set_meta("owner_id", player_owner)
	world.add_child(player)
	player.global_transform = world.spawn_point.global_transform
