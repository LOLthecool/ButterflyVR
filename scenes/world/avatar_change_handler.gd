extends MessageHandler
class_name AvatarChangeHandler

signal avatar_changed(player: PackedByteArray, avatar: UUID)


func send_message(player: PackedByteArray, avatar: UUID) -> void:
	var values: Array = [player, avatar.backing_storage]
	var types: Array[int] = []

	types.push_back(_get_value_type(null, 0))
	types.push_back(_get_value_type(values[0], 1))

	send_message_final(values, types)


func _get_value_type(_previous_value: Variant, idx: int) -> TypeHelper.NetworkedValueTypes:
	match idx:
		0:
			return TypeHelper.NetworkedValueTypes.ByteArray
		1:
			return TypeHelper.NetworkedValueTypes.ByteArray
	return TypeHelper.NetworkedValueTypes.End


func _process_message(values: Array) -> void:
	@warning_ignore("unsafe_cast")
	avatar_changed.emit(values[0], UUID.from_bytes(values[1] as PackedByteArray))
