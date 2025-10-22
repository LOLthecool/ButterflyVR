extends RefCounted
class_name UUID

var backing_storage:PackedByteArray

func _init(init:bool = true) -> void:
	if init:
		for _x:int in range(16):
			backing_storage.push_back(randi() % 256)
	else:
		for _x:int in range(16):
			backing_storage.push_back(0)

func _to_string() -> String:
	return '%02x%02x%02x%02x-%02x%02x-%02x%02x-%02x%02x-%02x%02x%02x%02x%02x%02x' % (backing_storage as Array[int])

static func from_String(uuid:String) -> UUID:
	var result:UUID = UUID.new(false)
	uuid = uuid.replace("-", "")
	if len(uuid) != 32: # 32 nibbles / 32 hex characters
		push_error("tried to parse invalid uuid")
		return result
	for i:int in range(0, 16):
		# every 2 hex character make a byte
		var byte:int = uuid.substr(i * 2, 2).hex_to_int()
		result.backing_storage[i] = byte
	return result

func as_array() -> Array[int]:
	return backing_storage as Array[int]
