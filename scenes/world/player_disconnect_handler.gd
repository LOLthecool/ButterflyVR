extends MessageHandler


func _physics_process(_delta: float) -> void:
	if NetworkManager.is_server():
		for player: PackedByteArray in NetworkManager.get_dc_clients():
			send_message_final([player], [_get_value_type(null, 0)])


func _get_value_type(_previous_value: Variant, idx: int) -> TypeHelper.NetworkedValueTypes:
	match idx:
		0:
			return TypeHelper.NetworkedValueTypes.ByteArray
	return TypeHelper.NetworkedValueTypes.End


func _process_message(values: Array) -> void:
	@warning_ignore("unsafe_cast")
	handle_on_dc(values[0] as PackedByteArray)


func handle_on_dc(player: PackedByteArray) -> void:
	for node: NetworkedNode in NetworkManager.get_networked_nodes():
		if node.owner_id == player and node.has_method("_on_owner_dc"):
			node._on_owner_dc()
