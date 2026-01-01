extends Node
class_name DownloadHandler



const OBJECT_INFO_ENDPOINT:String = "/api/v0/%s/%s"
const OBJECT_DOWNLOAD_ENDPOINT:String = "/api/v0/%s/%s/epck"
const MEGABYTE:int = 1024 * 1024
const GIGABYTE:int = MEGABYTE * 1024


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
	var result:Array[Variant] = GlobalAPIHandler.handle_response(response[0], 
			response[2], [200], ["encryption_key", "encryption_iv"])
	
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
	return decrypt_and_load_object(file, type, uuid, response_values["encryption_key"], 
			response_values["encryption_iv"])

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
	var result:Array[Variant] = GlobalAPIHandler.handle_response(response[0], 
			response[2], [200], ["updated_at", "object_size"])
	
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
			# get object just moved it to the front
			cache.pop_front()
	
	# cache value didnt exist or was stale so we download
	await download_object(uuid, type)
	var item:LRUCache.Pack = LRUCache.Pack.new(uuid, type, response_values["updated_at"], 
			response_values["object_size"] / 1024)
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
	
	var url:String = OBJECT_DOWNLOAD_ENDPOINT % [object_type_string, uuid]
	
	var downloader:HTTPRequest = HTTPRequest.new()
	add_child(downloader)
	
	downloader.download_file = cache.object_file_path % [object_type, uuid]
	
	if !DirAccess.dir_exists_absolute(
			cache.object_file_path.trim_suffix("%s.epck") % object_type):
		DirAccess.make_dir_recursive_absolute(
				cache.object_file_path.trim_suffix("%s.epck") % object_type)
	
	FileAccess.open(downloader.download_file, FileAccess.WRITE).close()
	
	downloader.request("http://" +
			GlobalAPIHandler.TARGET_HOST + ":" + str(GlobalAPIHandler.TARGET_PORT)
			 + url, PackedStringArray([GlobalAccountHandler.get_token_header()]))
	
	print(await downloader.request_completed)
	
	downloader.queue_free()

func decrypt_and_load_object(object:FileAccess, object_type:LRUCache.ObjectType, uuid:UUID, 
		key:PackedByteArray, iv:PackedByteArray) -> PackedScene:
	var aes:AESContext = AESContext.new()
	aes.start(AESContext.MODE_CBC_DECRYPT, key, iv)
	
	object.seek(0)
	
	var decrypted_buffer:PackedByteArray = PackedByteArray()
	
	while object.get_position() < object.get_length():
		# encrypted files should always be a multiple of 16 bytes long
		decrypted_buffer += aes.update(object.get_buffer(16))
	
	object.close()
	aes.finish()
	
	var new_object:FileAccess = FileAccess.create_temp(FileAccess.READ_WRITE, "object", ".pck", true)
	
	# trim pading bytes
	# padding bytes are 255 followed by 0s
	# there is always at least 1 padding byte (255)
	var zero_bytes:int = 0
	while decrypted_buffer[(decrypted_buffer.size() - zero_bytes) - 1] == 0:
		zero_bytes += 1
	
	if decrypted_buffer[(decrypted_buffer.size() - zero_bytes) - 1] != 255:
		push_error("object was not correctly padded, attempting to continue decoding")
		# if this is causing issues its probably safe to remove and just reject bad padding outright
		zero_bytes -= 1 # assumes the last byte of the object isnt 0, but this shouldnt happen anyways
	
	# remove all padding including 255
	decrypted_buffer.resize(decrypted_buffer.size() - (zero_bytes + 1))
	
	# todo: include uncompressed size when uploading
	decrypted_buffer = decrypted_buffer.decompress_dynamic(GIGABYTE * 4, FileAccess.COMPRESSION_GZIP)
	
	new_object.store_buffer(decrypted_buffer)
	new_object.flush()
	
	# not sure what would happen here if the os clears the temp file while we are running
	if !ProjectSettings.load_resource_pack(new_object.get_path(), false):
		push_error("failed to load object pck")
	
	new_object.close()
	
	return ResourceLoader.load("res://_loaded_content/%s/%s.tscn" % [object_type, uuid], 
			"PackedScene", ResourceLoader.CACHE_MODE_IGNORE_DEEP) as PackedScene
