extends Node
class_name DownloadHandler



const OBJECT_INFO_ENDPOINT:String = "/api/v0/%s/%s"
const OBJECT_DOWNLOAD_ENDPOINT:String = "/api/v0/%s/%s/epck"
const MEGABYTE:int = 1024 * 1024
const GIGABYTE:int = MEGABYTE * 1024

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
	@warning_ignore("unsafe_call_argument")
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
	
	var file:FileAccess = FileAccess.open(cache.object_file_path % [uuid], FileAccess.READ)
	@warning_ignore("unsafe_cast")
	return decrypt_and_load_object(file, type, uuid, response_values["encryption_key"] as PackedByteArray, 
			response_values["encryption_iv"] as PackedByteArray)

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
	@warning_ignore("unsafe_call_argument")
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
	
	var object:LRUCache.Pack = cache.get_object(uuid, type)
	if object:
		if object.cache_time_utc >= response_values["updated_at"]:
			return true
		else:
			cache.remove(uuid.to_string())
	
	# cache value didnt exist or was stale so we download
	await download_object(uuid, type)
	@warning_ignore("unsafe_cast")
	var item:LRUCache.Pack = LRUCache.Pack.new(response_values["updated_at"] as int, 
			int(ceilf(response_values["object_size"] as float / 1024)))
	cache.push_front(uuid.to_string(), item)
	return true

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
	
	downloader.download_file = cache.object_file_path % [uuid]
	
	if !DirAccess.dir_exists_absolute(
			cache.object_file_path.trim_suffix("%s.epck")):
		DirAccess.make_dir_recursive_absolute(
				cache.object_file_path.trim_suffix("%s.epck"))
	
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

func decrypt_and_load_object(object:FileAccess, object_type:LRUCache.ObjectType, uuid:UUID, 
		key:PackedByteArray, iv:PackedByteArray) -> PackedScene:
	var aes:AESContext = AESContext.new()
	aes.start(AESContext.MODE_CBC_DECRYPT, key, iv)
	
	object.seek(0)
	
	var decrypted_buffer:PackedByteArray = PackedByteArray()
	
	while object.get_position() + (1024 * 1024) < object.get_length():
		decrypted_buffer += aes.update(object.get_buffer(1024 * 1024))
	
	# encrypted files should always be a multiple of 16 bytes long
	decrypted_buffer += aes.update(object.get_buffer(object.get_length() - object.get_position()))
	
	object.close()
	aes.finish()
	
	var new_object:String = FileAccess.create_temp(FileAccess.READ_WRITE, "object", ".pck", true).get_path()
	
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
	
	var decrypted:FileAccess = FileAccess.create_temp(FileAccess.READ_WRITE, "object", ".pck", true)
	decrypted.store_buffer(decrypted_buffer)
	var decrypted_path:String = decrypted.get_path()
	decrypted.close()
	
	ZSTDCompressor.decompress_file_to_file(decrypted_path, new_object)
	
	# todo: a malicious object could contain files in _loaded_content/_/_ for another object uuid
	# since overwiting is forbidden (cant have then overwriting internal files) if that object is later loaded
	# it will use the malicious files. 
	# it will still need to follow the safety checks 
	# but this could allow bypassing a hypothetical future permission system for objects
	# by 'injecting' into an object with more permissions
	
	if !ProjectSettings.load_resource_pack(new_object, false):
		push_error("failed to load object pck from %s" % new_object)
	
	return ResourceLoader.load("res://_loaded_content/%s/%s.tscn" % [object_type, uuid], 
			"PackedScene", ResourceLoader.CACHE_MODE_IGNORE_DEEP) as PackedScene

func _physics_process(_delta: float) -> void:
	cache.process_destroy_queue()
