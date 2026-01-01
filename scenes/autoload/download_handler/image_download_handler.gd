extends Node
class_name ImageDownloadHandler
#todo: still uses the .epck extension for these files even though they are just images

const OBJECT_INFO_ENDPOINT:String = "api/v0/%s/%s"
const OBJECT_IMAGE_ENDPOINT:String = "api/v0/%s/%s/image"
const DAY_UTC:int = 60 * 60 * 24

# max size: 50mb
var cache:LRUCache = LRUCache.load_cache("image_cache", 1024 * 50, "images")

func get_object(uuid:UUID, type:LRUCache.ObjectType) -> Image:
	var response:Array[Variant] = await GlobalAPIHandler.make_request(
			HTTPClient.METHOD_GET, OBJECT_INFO_ENDPOINT % [uuid, type])
	var result:Array[Variant] = GlobalAPIHandler.handle_response(
			response[0], response[2], [200], ["updated_at", "image_size"])
	
	# todo: error handling
	var response_values:Dictionary[String, Variant] = result[4]
	
	if cache.has(uuid, type):
		if cache.get_object(uuid, type).cache_time_utc >= response_values["updated_at"]:
			return Image.load_from_file(cache.object_file_path % [type, uuid])
		else:
			cache.pop_front()
	
	# cache value didnt exist or was stale so we download
	download_object(uuid, type)
	var item:LRUCache.Pack = LRUCache.Pack.new(uuid, type, response_values["updated_at"], response_values["image_size"] / 1024)
	cache.push_front(item)
	return Image.load_from_file(cache.object_file_path % [type, uuid])

func download_object(uuid:UUID, object_type:LRUCache.ObjectType) -> void:
	var object_type_string:String = "UNNAMED"
	
	match object_type:
		LRUCache.ObjectType.world:
			object_type_string = "World"
		LRUCache.ObjectType.avatar:
			object_type_string = "Avatar"
	
	var url:String = OBJECT_IMAGE_ENDPOINT % [object_type_string, uuid]
	
	var downloader:HTTPRequest = HTTPRequest.new()
	add_child(downloader)
	
	downloader.download_file = cache.object_file_path % [uuid, object_type]
	
	if !DirAccess.dir_exists_absolute(
			cache.object_file_path.trim_suffix("%s.epck") % object_type):
		DirAccess.make_dir_recursive_absolute(
				cache.object_file_path.trim_suffix("%s.epck") % object_type)
	
	FileAccess.open(downloader.download_file, FileAccess.WRITE).close()
	
	downloader.request("http://" +
			GlobalAPIHandler.TARGET_HOST + ":" + str(GlobalAPIHandler.TARGET_PORT)
			 + url, PackedStringArray([GlobalAccountHandler.get_token_header()]))
	
	await downloader.request_completed
	
	downloader.queue_free()
