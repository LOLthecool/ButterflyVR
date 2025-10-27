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

class Pack:
	var identifier:PackIdentifier
	var cache_time_utc:int
	var size_KB:int
	
	var next:Pack
	var last:Pack
	
	func _init(uuid:UUID, object_type:ObjectType, cache_time_utc:int, size_KB:int) -> void:
		identifier = PackIdentifier.new(uuid, object_type)
		self.cache_time_utc = cache_time_utc
		self.size_KB = size_KB

const OBJECT_FILE_PATH:String = "user://cache/%s/%s.epck"

var cached_objects:Dictionary[PackIdentifier, Pack]
var cache_head:Pack
var cache_tail:Pack
var max_cache_size_KB:int = 500 * 1024 # 500MB
var cache_size_KB:int = 0

func push_front(item:Pack) -> void:
	if cache_head:
		item.last = cache_head
		cache_head.next = item
	else:
		cache_head = item
		cache_tail = item # if theres no head theres no tail either
	
	cached_objects[item.identifier] = item
	cache_size_KB += item.size_KB
	# if head == tail then popping would remove the item we just added
	while cache_size_KB > max_cache_size_KB and cache_head != cache_tail:
		pop_back()
	
	GlobalPersistanceHandler.set_value("cache_meta", "cache", "data", self)

func get_object(uuid:UUID, object_type:ObjectType) -> Pack:
	var identifier:PackIdentifier = PackIdentifier.new(uuid, object_type)
	if identifier in cached_objects:
		var object:Pack = cached_objects[identifier]
		
		if object.next:
			object.next.last = object.last
		else:
			cache_head = object.last
		
		if object.last:
			object.last.next = object.next
		else:
			cache_tail = object.next
		
		cached_objects.erase(identifier)
		
		GlobalPersistanceHandler.set_value("cache_meta", "cache", "data", self)
		
		return object
	return null

func has(uuid:UUID, object_type:ObjectType) -> bool:
	return cached_objects.has(PackIdentifier.new(uuid, object_type))

func pop_back() -> Pack:
	var tail:Pack = cache_tail
	if cache_tail:
		if cache_tail.next:
			cache_tail = cache_tail.next
			cache_tail.last = null
		else:
			cache_tail = null
			cache_head = null
	
	if tail:
		cached_objects.erase(tail.identifier)
		cache_size_KB -= tail.size_KB
		DirAccess.remove_absolute(OBJECT_FILE_PATH % [tail.identifier.object_type, tail.identifier.uuid])
	
	GlobalPersistanceHandler.set_value("cache_meta", "cache", "data", self)
	
	return tail

func pop_front() -> Pack:
	var head:Pack = cache_head
	if cache_head:
		if cache_head.last:
			cache_head = cache_head.last
			cache_head.next = null
		else:
			cache_head = null
			cache_tail = null
	
	if head:
		cached_objects.erase(head.identifier)
		cache_size_KB -= head.size_KB
		DirAccess.remove_absolute(OBJECT_FILE_PATH % [head.identifier.object_type, head.identifier.uuid])
	
	GlobalPersistanceHandler.set_value("cache_meta", "cache", "data", self)
	
	return head

func clear() -> void:
	while pop_back() != null:
		pass
	GlobalPersistanceHandler.set_value("cache_meta", "cache", "data", self)
