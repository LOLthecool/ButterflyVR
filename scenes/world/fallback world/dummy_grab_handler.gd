extends PlayerGrabHandler
# needed for player to not cause errors, does nothing

func send_message(_player:int, _target:String) -> void:
	return

func _get_value_type(_previous_value: Variant, _idx: int) -> int:
	return -1

func _process_message(_values: Array) -> void:
	return
