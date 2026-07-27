extends Node
class_name ImageDownloadHandler
#todo: still uses the .epck extension for these files even though they are just images

const OBJECT_INFO_ENDPOINT:String = "/api/v0/%s/%s"
const OBJECT_IMAGE_ENDPOINT:String = "/api/v0/%s/%s/image"
const DAY_UTC:int = 60 * 60 * 24
const MEGABYTE:int = 1024 * 1024
const CACHE_SIZE:int = MEGABYTE * 100
const CACHE_FILE:String = "image_cache"
const IMAGE_FILE_PATH:String = "user://images/%s"

var backing_cache:LruCache

func on_save(cached_objects:Dictionary) -> void:
	GlobalPersistanceHandler.clear_catagory(CACHE_FILE, "values", false)
	for uuid:String in cached_objects.keys():
		GlobalPersistanceHandler.register_value(
				CACHE_FILE, "values",uuid, cached_objects[uuid], false)
	GlobalPersistanceHandler.flush_file.call_deferred(CACHE_FILE)

func on_load() -> Dictionary:
	return GlobalPersistanceHandler.get_catagory(CACHE_FILE, "values")

func on_destroy(uuid:String) -> void:
	DirAccess.remove_absolute(IMAGE_FILE_PATH % uuid)

func _init() -> void:
	backing_cache = LruCache.new_cache(CACHE_SIZE, on_save, on_load, on_destroy)
	backing_cache.load()

func get_object(uuid:UUID, type:TypeHelper.ObjectType) -> Image:
	var id:String = uuid.to_string()
	var object_type_string:String = "UNNAMED"
	
	match type:
		TypeHelper.ObjectType.world:
			object_type_string = "World"
		TypeHelper.ObjectType.avatar:
			object_type_string = "Avatar"
	
	var response:Array[Variant] = await GlobalAPIHandler.make_request(
			HTTPClient.METHOD_GET, OBJECT_INFO_ENDPOINT % [object_type_string, uuid], 
			PackedStringArray([GlobalAccountHandler.get_token_header()]))
	@warning_ignore("unsafe_call_argument")
	var result:Array[Variant] = GlobalAPIHandler.handle_response(
			response[0], response[2], [200], ["updated_at", "image_size"])
	
	# todo: error handling
	var response_values:Dictionary[String, Variant] = result[4]
	
	var image:Dictionary[String, int] = {}
	image.assign(backing_cache.get(id))
	if !image.is_empty():
		if (!FileAccess.file_exists(IMAGE_FILE_PATH % [id])) or \
				FileAccess.get_size(IMAGE_FILE_PATH % [id]) < 1:
			push_error("cached file did not exist for object image: %s" % id)
			backing_cache.pop(id)
		else:
			if image.cache_time_utc >= response_values["updated_at"]:
				backing_cache.save()
				return load_image(IMAGE_FILE_PATH % id)
			else:
				backing_cache.pop(id)
	
	# cache value didnt exist or was stale so we download
	await download_object(id, type)
	@warning_ignore("unsafe_cast")
	backing_cache.push_front(id, 
			response_values["updated_at"] as int, 
			int(ceilf(response_values["image_size"] as float / 1024)))
	backing_cache.save()
	return load_image(IMAGE_FILE_PATH % [uuid])

func load_image(file:String) -> Image:
	var buffer:PackedByteArray = FileAccess.get_file_as_bytes(file)
	var new_image:Image = Image.new()
	# todo: stop these from emitting errors whenever we load an image
	if new_image.load_png_from_buffer(buffer) == OK:
		return new_image
	elif new_image.load_jpg_from_buffer(buffer) == OK:
		return new_image
	elif new_image.load_webp_from_buffer(buffer) == OK:
		return new_image
	else:
		push_error("failed to parse image file.")
		return new_image

func download_object(uuid:String, object_type:TypeHelper.ObjectType) -> void:
	var object_type_string:String = "UNNAMED"
	
	match object_type:
		TypeHelper.ObjectType.world:
			object_type_string = "World"
		TypeHelper.ObjectType.avatar:
			object_type_string = "Avatar"
	
	var url:String = OBJECT_IMAGE_ENDPOINT % [object_type_string, uuid]
	
	var downloader:HTTPRequest = HTTPRequest.new()
	add_child(downloader)
	
	downloader.download_file = IMAGE_FILE_PATH % [uuid]
	
	if !DirAccess.dir_exists_absolute(
			IMAGE_FILE_PATH.trim_suffix("%s")):
		DirAccess.make_dir_recursive_absolute(
				IMAGE_FILE_PATH.trim_suffix("%s"))
	
	FileAccess.open(downloader.download_file, FileAccess.WRITE).close()
	
	if GlobalAPIHandler.target_port == 443:
		downloader.request("https://" +
				GlobalAPIHandler.target_host + ":" + str(GlobalAPIHandler.target_port)
				 + url, PackedStringArray([GlobalAccountHandler.get_token_header()]))
	else:
		downloader.request("http://" +
				GlobalAPIHandler.target_host + ":" + str(GlobalAPIHandler.target_port)
				 + url, PackedStringArray([GlobalAccountHandler.get_token_header()]))
	
	await downloader.request_completed
	
	downloader.queue_free()

func _physics_process(_delta: float) -> void:
	backing_cache.process_destroy_queue()
