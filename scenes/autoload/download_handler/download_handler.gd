extends Node
class_name DownloadHandler



const OBJECT_INFO_ENDPOINT:String = "/api/v0/%s/%s"
const OBJECT_DOWNLOAD_ENDPOINT:String = "/api/v0/%s/%s/epck"
const MEGABYTE:int = 1024
const GIGABYTE:int = 1024 * 1024


# max size: 10GB
var cache:LRUCache = LRUCache.load_cache("cache_meta", GIGABYTE * 10, "cache")

func get_object(uuid:UUID, type:LRUCache.ObjectType) -> PackedScene:
	if !await preload_object(uuid, type):
		push_warning("error in preload step, returning null")
		return null
	
	var object_type_string:String = "UNNAMED"
	match type:
		LRUCache.ObjectType.world:
			object_type_string = "World"
		LRUCache.ObjectType.avatar:
			object_type_string = "Avatar"
	
	var response:Array[Variant] = await GlobalAPIHandler.make_request(
			HTTPClient.METHOD_GET, 
			OBJECT_INFO_ENDPOINT % [object_type_string, uuid],
			PackedStringArray([GlobalAccountHandler.get_token_header()]))
	var result:Array[Variant] = GlobalAPIHandler.handle_response(response[0], response[2], [200], ["key", "iv"])
	
	var success:bool = result[0]
	var response_code:int = result[1]
	var error_code:String = result[2]
	var error_message:String = result[3]
	var response_values:Dictionary[String, Variant] = result[4]
	
	if (!success):
		push_warning("failed to aquire object data")
		if response_code != -1:
			push_error("server response: %s" % response_code)
		if error_code != "":
			push_error("error code: %s" % error_code)
		if error_message != "":
			push_error("error message: %s" % error_message)
		return null
	
	var file:FileAccess = FileAccess.open(cache.object_file_path % [type, uuid], FileAccess.READ)
	return decrypt_and_load_object(file, type, uuid, response_values["key"], response_values["iv"])

func preload_object(uuid:UUID, type:LRUCache.ObjectType) -> bool:
	var object_type_string:String = "UNNAMED"
	
	match type:
		LRUCache.ObjectType.world:
			object_type_string = "World"
		LRUCache.ObjectType.avatar:
			object_type_string = "Avatar"
	
	var response:Array[Variant] = await GlobalAPIHandler.make_request(
			HTTPClient.METHOD_GET, 
			OBJECT_INFO_ENDPOINT % [object_type_string, uuid],
			PackedStringArray([GlobalAccountHandler.get_token_header()]))
	var result:Array[Variant] = GlobalAPIHandler.handle_response(response[0], response[2], [200], ["updated_at", "object_size"])
	
	var success:bool = result[0]
	var response_code:int = result[1]
	var error_code:String = result[2]
	var error_message:String = result[3]
	var response_values:Dictionary[String, Variant] = result[4]
	
	if (!success):
		push_warning("failed to aquire object data")
		if response_code != -1:
			push_error("server response: %s" % response_code)
		if error_code != "":
			push_error("error code: %s" % error_code)
		if error_message != "":
			push_error("error message: %s" % error_message)
		return false
	
	if cache.has(uuid, type):
		if cache.get_object(uuid, type).cache_time_utc >= response_values["updated_at"]:
			return true
		else:
			cache.pop_front()
	
	# cache value didnt exist or was stale so we download
	download_object(uuid, type)
	var item = LRUCache.Pack.new(uuid, type, response_values["updated_at"], response_values["object_size"])
	cache.push_front(item)
	return true

func remove_all_expired() -> void:
	## todo
	push_error("not yet implemented")

func download_object(uuid:UUID, object_type:LRUCache.ObjectType) -> void:
	var object_type_string:String = "UNNAMED"
	
	match object_type:
		LRUCache.ObjectType.world:
			object_type_string = "World"
		LRUCache.ObjectType.avatar:
			object_type_string = "Avatar"
	
	var url = OBJECT_DOWNLOAD_ENDPOINT % [object_type_string, uuid]
	
	var downloader:HTTPRequest = HTTPRequest.new()
	add_child(downloader)
	
	downloader.download_file = cache.object_file_path % [uuid, object_type]
	downloader.request("http://" +
			GlobalAPIHandler.TARGET_HOST + ":" + str(GlobalAPIHandler.TARGET_PORT)
			 + url, PackedStringArray([GlobalAccountHandler.get_token_header()]))
	
	await downloader.request_completed
	
	downloader.queue_free()

func decrypt_and_load_object(object:FileAccess, object_type:LRUCache.ObjectType, uuid:UUID, key:PackedByteArray, iv:PackedByteArray) -> PackedScene:
	var aes:AESContext = AESContext.new()
	aes.start(AESContext.MODE_CBC_DECRYPT, key, iv)
	
	var decrypted_buffer:PackedByteArray = PackedByteArray()
	
	while object.get_position() < object.get_length():
		# encrypted files should always be a multiple of 16 bytes long
		decrypted_buffer += aes.update(object.get_buffer(16))
		object.seek(object.get_position() + 16)
	
	object.close()
	aes.finish()
	
	var new_object:FileAccess = FileAccess.create_temp(FileAccess.READ_WRITE, "object", ".pck", true)
	
	# not sure what would happen here if the os clears the temp file while we are running
	ProjectSettings.load_resource_pack(new_object.get_path(), false)
	
	new_object.close()
	return load("res://_loaded_content/%s/%s" % [object_type, uuid]) as PackedScene
