extends RefCounted
class_name UUID

var backing_storage:PackedByteArray

func _init(randomize:bool = false) -> void:
	if randomize:
		for _x:int in range(16):
			backing_storage.push_back(randi() % 256)
	else:
		for _x:int in range(16):
			backing_storage.push_back(0)

func _to_string() -> String:
	return '%02x%02x%02x%02x-%02x%02x-%02x%02x-%02x%02x-%02x%02x%02x%02x%02x%02x' % (backing_storage as Array[int])

# operator overloading when?
func equals(other:UUID) -> bool:
	return self.backing_storage == other.backing_storage

static func from_String(uuid:String) -> UUID:
	var result:UUID = UUID.new()
	
	uuid = uuid.replace("-", "")
	
	if len(uuid) != 32 or !uuid.is_valid_hex_number(): # 32 nibbles / 32 hex characters
		push_error("tried to parse invalid uuid")
		return result
	
	for i:int in range(0, 16):
		# every 2 hex character make a byte
		var byte:int = uuid.substr(i * 2, 2).hex_to_int()
		result.backing_storage[i] = byte
	
	return result

static func from_bytes(bytes:PackedByteArray) -> UUID:
	var result = UUID.new()
	if bytes.size() != 16:
		push_error("uuid bytes array was wrong size")
		return result
	result.backing_storage = bytes
	return result

func as_array() -> Array[int]:
	return backing_storage as Array[int]
