extends MessageHandler
class_name PlayerGrabHandler

signal player_grabbed(player:PackedByteArray, object:Node)

func send_message(player:PackedByteArray, target:String) -> void:
	var index_path:Array[int] = []
	if target != "":
		index_path = PathHelper.path_to_index_path(target, self)
		if index_path.is_empty():
			push_error("failed to parse path of grabbed node")
			return
	
	
	var values:Array = [player, index_path]
	var types:Array[int] = []
	
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
	var target:Node = get_tree().root
	if values[1] != []:
		@warning_ignore("unsafe_cast")
		var indexes:Array[int] = (values[1] as Array[int])
		indexes.reverse()
		# scene tree could be desynced for us so dont blindly trust the path
		for idx:int in values[1]:
			if target.get_child(idx) == null:
				push_warning("failed to grab node")
				return
			target = target.get_child(idx)
	else:
		target = null
	
	player_grabbed.emit(values[0], target)
