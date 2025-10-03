extends RefCounted
class_name UUID

var backing_storage:PackedByteArray

func _init(init:bool = true) -> void:
	if init:
		for _x in range(16):
			backing_storage.push_back(randi() % 256)
	else:
		for _x in range(16):
			backing_storage.push_back(0)

func _to_string() -> String:
	return '%02x%02x%02x%02x-%02x%02x-%02x%02x-%02x%02x-%02x%02x%02x%02x%02x%02x' % (backing_storage as Array[int])

func from_String(uuid:String) -> UUID:
	var result = UUID.new(false)
	uuid = uuid.replace("-", "")
	if len(uuid) != 32: # 32 nibbles / 32 hex characters
		push_error("tried to parse invalid uuid")
		return result
	for i in range(0, 32, 2):
		var byte = uuid.substr(i, 2).hex_to_int()
		result.backing_storage[i / 2] = byte
	return result

func as_array():
	return backing_storage as Array[int]
