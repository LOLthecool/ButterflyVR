extends Node
class_name ImageDownloadHandler
#todo: still uses the .epck extension for these files even though they are just images

const OBJECT_INFO_ENDPOINT:String = "/api/v0/%s/%s"
const OBJECT_IMAGE_ENDPOINT:String = "/api/v0/%s/%s/image"
const DAY_UTC:int = 60 * 60 * 24

# max size: 50mb
var cache:LRUCache = LRUCache.load_cache("image_cache", 1024 * 50, "images")


func get_object(uuid:UUID, type:LRUCache.ObjectType) -> Image:
	var object_type_string:String = "UNNAMED"
	
	match type:
		LRUCache.ObjectType.world:
			object_type_string = "World"
		LRUCache.ObjectType.avatar:
			object_type_string = "Avatar"
	
	var response:Array[Variant] = await GlobalAPIHandler.make_request(
			HTTPClient.METHOD_GET, OBJECT_INFO_ENDPOINT % [object_type_string, uuid], 
			PackedStringArray([GlobalAccountHandler.get_token_header()]))
	var result:Array[Variant] = GlobalAPIHandler.handle_response(
			response[0], response[2], [200], ["updated_at", "image_size"])
	
	# todo: error handling
	var response_values:Dictionary[String, Variant] = result[4]
	
	if cache.has(uuid, type):
		if cache.get_object(uuid, type).cache_time_utc >= response_values["updated_at"]:
			return load_image(cache.object_file_path % [type, uuid])
		else:
			cache.pop_front()
	
	# cache value didnt exist or was stale so we download
	await download_object(uuid, type)
	var identifier:LRUCache.PackIdentifier = LRUCache.PackIdentifier.new(uuid, type)
	var item:LRUCache.Pack = LRUCache.Pack.new(identifier, response_values["updated_at"], response_values["image_size"] / 1024)
	cache.push_front(item)
	return load_image(cache.object_file_path % [type, uuid])

func load_image(file:String) -> Image:
	var buffer:PackedByteArray = FileAccess.get_file_as_bytes(file)
	var new_image:Image = Image.new()
	if new_image.load_png_from_buffer(buffer) == OK:
		return new_image
	elif new_image.load_jpg_from_buffer(buffer) == OK:
		return new_image
	elif new_image.load_webp_from_buffer(buffer) == OK:
		return new_image
	else:
		push_error("failed to parse image file.")
		return new_image

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
	
	downloader.download_file = cache.object_file_path % [object_type, uuid]
	
	if !DirAccess.dir_exists_absolute(
			cache.object_file_path.trim_suffix("%s.epck") % object_type):
		DirAccess.make_dir_recursive_absolute(
				cache.object_file_path.trim_suffix("%s.epck") % object_type)
	
	FileAccess.open(downloader.download_file, FileAccess.WRITE).close()
	
	downloader.request("https://" +
			GlobalAPIHandler.TARGET_HOST + ":" + str(GlobalAPIHandler.TARGET_PORT)
			 + url, PackedStringArray([GlobalAccountHandler.get_token_header()]))
	
	await downloader.request_completed
	
	downloader.queue_free()
