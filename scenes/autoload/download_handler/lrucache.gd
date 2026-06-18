extends RefCounted
# wrapper over the rust lru
# todo: cut this out and just use the rust directly
class_name LRUCache

# todo: move this somewhere else
enum ObjectType{
	world,
	avatar,
	prop,
	component
}

class Pack:
	var cache_time_utc:int
	var size_KB:int
	
	func _init(cache_time_utc:int, size_KB:int) -> void:
		self.cache_time_utc = cache_time_utc
		self.size_KB = size_KB

const BASE_OBJECT_FILE_PATH:String = "user://%s/%s.epck"

var cache_file:String
var object_file_path:String
var backing_cache:LruCache

func process_destroy_queue() -> void:
	backing_cache.process_destroy_queue()

func on_save(cached_objects:Dictionary) -> void:
	GlobalPersistanceHandler.clear_catagory(cache_file, "values")
	for uuid:String in cached_objects.keys():
		GlobalPersistanceHandler.save_value(
				cache_file, 
				"values",
				uuid, 
				cached_objects[uuid])

func on_load() -> Dictionary:
	return GlobalPersistanceHandler.get_catagory(cache_file, "values")

func on_destroy(uuid:String) -> void:
	DirAccess.remove_absolute(object_file_path % uuid)

# todo: max size changes only take effect next time something is loaded
static func load_cache(file:String, max_size:int, cache_name:String) -> LRUCache:
	var object_file_path:String = BASE_OBJECT_FILE_PATH % [cache_name, "%s"]
	
	var cache:LRUCache = LRUCache.new()
	var backing_cache:LruCache = LruCache.new_cache(
			max_size, cache.on_save, cache.on_load, cache.on_destroy)
	cache.backing_cache = backing_cache
	cache.cache_file = file
	cache.object_file_path = object_file_path
	backing_cache.load()
	return cache

func push_front(uuid:String, item:Pack) -> void:
	backing_cache.push_front(uuid, item.cache_time_utc, item.size_KB)
	backing_cache.save()

# todo: object_type is now redundant and should be removed and uuid should be changed to String
func get_object(uuid:UUID, _object_type:ObjectType) -> Pack:
	var result:Dictionary[String, int] = {}
	result.assign(backing_cache.get(uuid.to_string()))
	if result.is_empty():
		return null
	return Pack.new(result["cache_time_utc"], result["size_kb"])

func remove(uuid:String) -> void:
	backing_cache.pop(uuid)
	backing_cache.save()
