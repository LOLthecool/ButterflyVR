extends Node
class_name DownloadHandler

enum ObjectType{
	world,
	avatar,
	prop,
	component
}

# handles downloads seperately from api since api handler cant download to file
var downloaders:Array[HTTPRequest]
var cache:CacheLinkedHashSet

class CacheLinkedHashSet:
	var cached_objects:Dictionary[PackIdentifier, Pack]
	var cache_head:Pack
	var cache_tail:Pack
	var max_cache_size_KB:int = 500 * 1024 # 500MB
	
	func push_front(item:Pack) -> void:
		if cache_head:
			item.last = cache_head
			cache_head.next = item
		else:
			cache_head = item
			cache_tail = item # if theres no head theres no tail either
		cached_objects[item.identifier] = item
	
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
			return object
		return null
	
	func pop() -> Pack:
		var tail:Pack = cache_tail
		if cache_tail.next:
			cache_tail = cache_tail.next
			cache_tail.last = null
		else:
			cache_tail = null
			cache_head = null
		cached_objects.erase(tail.identifier)
		return tail

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

func get_object(uuid:UUID, type:ObjectType, last_update_utc:int) -> PackedScene:
	return null

func preload_object(uuid:UUID, last_update_utc:int) -> void:
	pass

func remove_if_expired(uuid:UUID, last_update_utc:int) -> void:
	pass
