extends AvatarChangeHandler
# needed for player to not cause errors, does nothing


func send_message(_player: PackedByteArray, _avatar: UUID) -> void:
	return


func _get_value_type(_previous_value: Variant, _idx: int) -> TypeHelper.NetworkedValueTypes:
	return TypeHelper.NetworkedValueTypes.End


func _process_message(_values: Array) -> void:
	return
