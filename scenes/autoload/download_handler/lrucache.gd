extends Node
# cache used in the download manager
# moved to a seperate file since its non trivial
class_name LRUCache

# todo: move this somewhere else since its used everywhere and isnt even used here anymore
enum ObjectType{
	world,
	avatar,
	prop,
	component
}

class Pack:
	var cache_time_utc:int
	var size_KB:int
	
	var next_pack_uuid:String
	var last_pack_uuid:String
	
	func _init(cache_time_utc:int, size_KB:int, next_pack_uuid:String = "", last_pack_uuid:String = "") -> void:
		self.cache_time_utc = cache_time_utc
		self.size_KB = size_KB
		self.next_pack_uuid = next_pack_uuid
		self.last_pack_uuid = last_pack_uuid
	
	func _to_string() -> String:
		return "%s,%s,%s,%s" % [str(cache_time_utc), str(size_KB), 
				str(next_pack_uuid), str(last_pack_uuid)]
	
	static func from_string(string:String) -> Pack:
		var strings:PackedStringArray = string.split(",", true, 3)
		var cache_time_utc:int = int(strings[0])
		var size_KB:int = int(strings[1])
		var next_pack_uuid:String = strings[2]
		var last_pack_uuid:String = strings[3]
		return Pack.new(cache_time_utc, size_KB, next_pack_uuid, last_pack_uuid)

const BASE_OBJECT_FILE_PATH:String = "user://%s/%s.epck"

var cached_objects:Dictionary[String, Pack]
var cache_head:String
var cache_tail:String
var cache_size_KB:int
var cache_file:String
var object_file_path:String
var max_cache_size:int

func save_self() -> void:
	GlobalPersistanceHandler.clear_catagory(cache_file, "values")
	for uuid:String in cached_objects.keys():
		GlobalPersistanceHandler.save_value(cache_file, "values", uuid, cached_objects[uuid].to_string())

# todo: max size changes only take effect next time something is loaded
static func load_cache(file:String, max_size:int, cache_name:String) -> LRUCache:
	var new_backing_store:Dictionary[String, Pack]
	
	var head:String = ""
	var tail:String = ""
	
	var stored_values:Dictionary[String, String] = {}
	var stored_size:int = 0
	var object_file_path:String = BASE_OBJECT_FILE_PATH % [cache_name, "%s"]
	
	stored_values.assign(GlobalPersistanceHandler.get_catagory(file, "values"))
	
	for stored_uuid:String in stored_values.keys():
		var stored_pack:Pack = Pack.from_string(stored_values[stored_uuid])
		
		if stored_pack.next_pack_uuid == "":
			if head != "":
				push_error("got duplicate heads: %s and %s" % [stored_pack, head])
			else:
				head = stored_uuid
		if stored_pack.last_pack_uuid == "":
			if tail != "":
				push_error("got duplicate tails: %s and %s" % [stored_pack, head])
			else:
				tail = stored_uuid
		
		stored_size += stored_pack.size_KB
		
		new_backing_store[stored_uuid] = stored_pack
	
	var cache:LRUCache = LRUCache.new()
	cache.cached_objects = new_backing_store
	cache.cache_head = head
	cache.cache_tail = tail
	cache.cache_size_KB = stored_size
	cache.cache_file = file
	cache.object_file_path = object_file_path
	cache.max_cache_size = max_size
	return cache

func push_front(uuid:String, item:Pack) -> void:
	if cache_head:
		item.last_pack_uuid = cache_head
		cached_objects[cache_head].next_pack_uuid = uuid
		cache_head = uuid
	else:
		cache_head = uuid
		cache_tail = uuid # if theres no head theres no tail either
	
	cached_objects[uuid] = item
	cache_size_KB += item.size_KB
	# if head == tail then popping would remove the item we just added
	while cache_size_KB > max_cache_size and cache_head != cache_tail:
		pop_back()
	
	save_self()

# todo: object_type is now redundant and should be removed and uuid should be changed to String
func get_object(uuid_:UUID, _object_type:ObjectType) -> Pack:
	var uuid:String = uuid_.to_string()
	if cached_objects.has(uuid):
		var object:Pack = cached_objects[uuid]
		
		if object.next_pack_uuid == "":
			# object is already the head so we can just return it directly
			return object
		
		cached_objects[object.next_pack_uuid].last_pack_uuid = object.last_pack_uuid
		
		if object.last_pack_uuid != "":
			cached_objects[object.last_pack_uuid].next_pack_uuid = object.next_pack_uuid
		else:
			cache_tail = object.next_pack_uuid
		
		cached_objects.erase(uuid)
		
		push_front(uuid, object)
		
		save_self()
		
		return object
	else:
		return null

func pop_back() -> Pack:
	if cache_tail != "":
		var tail:Pack = cached_objects[cache_tail]
		if cached_objects[cache_tail].next_pack_uuid != "":
			cache_tail = cached_objects[cache_tail].next_pack_uuid
			cached_objects[cache_tail].last_pack_uuid = ""
		else:
			cache_tail = ""
			cache_head = ""
		
		var uuid:String = cached_objects.find_key(tail)
		
		cached_objects.erase(uuid)
		cache_size_KB -= tail.size_KB
		DirAccess.remove_absolute(object_file_path % uuid)
		
		save_self()
		
		return tail
	return null

func pop_front() -> Pack:
	if cache_head != "":
		var head:Pack = cached_objects[cache_head]
		if cached_objects[cache_head].last_pack_uuid != "":
			cache_head = cached_objects[cache_head].last_pack_uuid
			cached_objects[cache_head].next_pack_uuid = ""
		else:
			cache_tail = ""
			cache_head = ""
		
		var uuid:String = cached_objects.find_key(head)
		
		cached_objects.erase(uuid)
		cache_size_KB -= head.size_KB
		DirAccess.remove_absolute(object_file_path % uuid)
		
		save_self()
		
		return head
	return null

func clear() -> void:
	while pop_back() != null:
		pass
	save_self()
