extends Node
# cache used in the download manager
# moved to a seperate file to avoid bloat
class_name LRUCache

enum ObjectType{
	world,
	avatar,
	prop,
	component
}

class PackIdentifier:
	var uuid:UUID
	var object_type:ObjectType
	func _init(uuid:UUID, object_type:ObjectType) -> void:
		self.uuid = uuid
		self.object_type = object_type
	
	func _to_string() -> String:
		return "%s - %s" % [object_type, uuid]
	
	static func from_string(string:String) -> PackIdentifier:
		var values:PackedStringArray = string.split(" - ", false, 1)
		return PackIdentifier.new(UUID.from_String(values[1]), int(values[0]))

class Pack:
	var identifier:PackIdentifier
	var cache_time_utc:int
	var size_KB:int
	
	var next:PackIdentifier
	var last:PackIdentifier
	
	func _init(uuid:UUID, object_type:ObjectType, cache_time_utc:int, size_KB:int) -> void:
		identifier = PackIdentifier.new(uuid, object_type)
		self.cache_time_utc = cache_time_utc
		self.size_KB = size_KB
	
	func as_json() -> String:
		return JSON.stringify({
			"cache_time_utc": cache_time_utc, 
			"size_KB": size_KB, 
			"next": next.to_string(),
			"last": last.to_string()})
	
	static func from_json(identifier:PackIdentifier, json:String) -> Pack:
		var values:Dictionary[String, Variant] = JSON.parse_string(json)
		return Pack.new(identifier.uuid, 
				identifier.object_type, 
				values["cache_time_utc"], 
				values["size_KB"])

const BASE_OBJECT_FILE_PATH:String = "user://%s/%s/%s.epck"

var cached_objects:Dictionary[PackIdentifier, Pack]
var cache_head:Pack
var cache_tail:Pack
var cache_size_KB:int = 0
var cache_file:String
var object_file_path:String
var max_cache_size:int

func save_self() -> void:
	GlobalPersistanceHandler.register_value(cache_file, "metadata", "sizeKB", cache_size_KB)
	GlobalPersistanceHandler.register_value(cache_file, "metadata", "cache_path", object_file_path)
	GlobalPersistanceHandler.clear_catagory(cache_file, "values")
	for key:PackIdentifier in cached_objects.keys():
		GlobalPersistanceHandler.register_value(cache_file, "values", key.to_string(), cached_objects[key].as_json())

# todo: max size changes only take effect next time something is loaded
static func load_cache(file:String, max_size:int, default_cache_name:String) -> LRUCache:
	var new_backing_store:Dictionary[PackIdentifier, Pack]
	
	var first_map:Dictionary[PackIdentifier, PackIdentifier]
	var last_map:Dictionary[PackIdentifier, PackIdentifier]
	
	var head:Pack
	var tail:Pack
	
	var stored_values:Dictionary[String, String] = {}
	
	var stored_size:int = GlobalPersistanceHandler.register_value(file, "metadata", "sizeKB", 0)
	var object_file_path:String = GlobalPersistanceHandler.register_value(
			file, "metadata", "cache_path", BASE_OBJECT_FILE_PATH % [default_cache_name, "%s", "%s"])
	
	stored_values.assign(GlobalPersistanceHandler.get_catagory(file, "values"))
	
	for key:PackIdentifier in stored_values.keys().map(func(x:String) -> PackIdentifier: return PackIdentifier.from_string(x)):
		new_backing_store[key] = Pack.from_json(key, stored_values[key.to_string()])
		
		if JSON.parse_string(stored_values[key.to_string()]).has("next"):
			first_map[key] = JSON.parse_string(stored_values[key.to_string()])["next"]
		else:
			head = new_backing_store[key]
		
		if JSON.parse_string(stored_values[key.to_string()]).has("last"):
			last_map[key] = JSON.parse_string(stored_values[key.to_string()])["last"]
		else:
			tail = new_backing_store[key]
	
	for key:PackIdentifier in new_backing_store.keys():
		new_backing_store[key].next = new_backing_store[first_map[key]].identifier
		new_backing_store[key].last = new_backing_store[last_map[key]].identifier
	
	var cache:LRUCache = LRUCache.new()
	cache.cached_objects = new_backing_store
	cache.cache_head = head
	cache.cache_tail = tail
	cache.cache_size_KB = stored_size
	cache.cache_file = file
	cache.object_file_path = object_file_path
	cache.max_cache_size = max_size
	return cache

func push_front(item:Pack) -> void:
	if cache_head:
		item.last = cache_head.identifier
		cache_head.next = item.identifier
	else:
		cache_head = item
		cache_tail = item # if theres no head theres no tail either
	
	cached_objects[item.identifier] = item
	cache_size_KB += item.size_KB
	# if head == tail then popping would remove the item we just added
	while cache_size_KB > max_cache_size and cache_head != cache_tail:
		pop_back()
	
	GlobalPersistanceHandler.set_value(cache_file, "cache", "data", self)

func get_object(uuid:UUID, object_type:ObjectType) -> Pack:
	var identifier:PackIdentifier = PackIdentifier.new(uuid, object_type)
	if identifier in cached_objects:
		var object:Pack = cached_objects[identifier]
		
		if object.next:
			cached_objects[object.next].last = object.last
		else:
			cache_head = cached_objects[object.last]
		
		if object.last:
			cached_objects[object.last].next = object.next
		else:
			cache_tail = cached_objects[object.next]
		
		cached_objects.erase(identifier)
		
		GlobalPersistanceHandler.set_value(cache_file, "cache", "data", self)
		
		return object
	return null

func has(uuid:UUID, object_type:ObjectType) -> bool:
	return cached_objects.has(PackIdentifier.new(uuid, object_type))

func pop_back() -> Pack:
	var tail:Pack = cache_tail
	if cache_tail:
		if cache_tail.next:
			cache_tail = cached_objects[cache_tail.next]
			cache_tail.last = null
		else:
			cache_tail = null
			cache_head = null
	
	if tail:
		cached_objects.erase(tail.identifier)
		cache_size_KB -= tail.size_KB
		DirAccess.remove_absolute(object_file_path % [tail.identifier.object_type, tail.identifier.uuid])
	
	GlobalPersistanceHandler.set_value(cache_file, "cache", "data", self)
	
	return tail

func pop_front() -> Pack:
	var head:Pack = cache_head
	if cache_head:
		if cache_head.last:
			cache_head = cached_objects[cache_head.last]
			cache_head.next = null
		else:
			cache_head = null
			cache_tail = null
	
	if head:
		cached_objects.erase(head.identifier)
		cache_size_KB -= head.size_KB
		DirAccess.remove_absolute(object_file_path % [head.identifier.object_type, head.identifier.uuid])
	
	GlobalPersistanceHandler.set_value(cache_file, "cache", "data", self)
	
	return head

func clear() -> void:
	while pop_back() != null:
		pass
	GlobalPersistanceHandler.set_value(cache_file, "cache", "data", self)
