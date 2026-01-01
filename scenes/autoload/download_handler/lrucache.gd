extends Node
# cache used in the download manager
# moved to a seperate file to avoid bloat
class_name LRUCache

# todo: move this somewhere else since its used everywhere
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
		var dict:Dictionary[String, Variant] = {
			"cache_time_utc": cache_time_utc, 
			"size_KB": size_KB}
		
		if next:
			dict["next"] = next.to_string()
		if last:
			dict["last"] = last.to_string()
		
		return JSON.stringify(dict)
	
	static func from_json(identifier:PackIdentifier, json:String) -> Pack:
		var values:Dictionary[String, Variant] = {}
		values.assign(JSON.parse_string(json))
		
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
		# for some reason these need to be their own variables
		# if you can remove them without breaking everything feel free
		var x:String = key.to_string()
		var y:String = cached_objects[key].as_json()
		GlobalPersistanceHandler.register_value(cache_file, "values", x, y)

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
			first_map[key] = PackIdentifier.from_string(JSON.parse_string(stored_values[key.to_string()])["next"])
		else:
			head = new_backing_store[key]
		
		if JSON.parse_string(stored_values[key.to_string()]).has("last"):
			last_map[key] = PackIdentifier.from_string(JSON.parse_string(stored_values[key.to_string()])["last"])
		else:
			tail = new_backing_store[key]
	
	for key:PackIdentifier in new_backing_store.keys():
		if first_map.has(key):
			var y:PackIdentifier = first_map[key]
			var x:Pack = new_backing_store[y]
			new_backing_store[key].next = x.identifier
		if last_map.has(key):
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
	
	save_self()

func get_object(uuid:UUID, object_type:ObjectType) -> Pack:
	if has(uuid, object_type):
		var object:Pack
		for i:Pack in cached_objects.values():
			if i.identifier.uuid.equals(uuid) and i.identifier.object_type == object_type:
				object = i
				break
		
		if object.next:
			cached_objects[object.next].last = object.last
		else:
			if object.last:
				cache_head = cached_objects[object.last]
			else:
				cache_head = null
		
		if object.last:
			cached_objects[object.last].next = object.next
		else:
			if object.next:
				cache_tail = cached_objects[object.next]
			else:
				cache_tail = null
		
		cached_objects.erase(object.identifier)
		
		push_front(object)
		
		save_self()
		
		return object
	return null

func has(uuid:UUID, object_type:ObjectType) -> bool:
	for object:PackIdentifier in cached_objects.keys():
		if object.uuid.equals(uuid) and object.object_type == object_type:
			return true
	return false

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
	
	save_self()
	
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
	
	save_self()
	
	return head

func clear() -> void:
	while pop_back() != null:
		pass
	save_self()
