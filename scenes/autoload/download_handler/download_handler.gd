extends Node
class_name DownloadHandler



const OBJECT_INFO_ENDPOINT:String = "api/v0/%s/%s"
const OBJECT_DOWNLOAD_ENDPOINT:String = "api/v0/%s/%s/epck"
const MEGABYTE:int = 1024
const GIGABYTE:int = 1024 * 1024


# max size: 10GB
var cache:LRUCache = LRUCache.load_cache("cache_meta", GIGABYTE * 10, "cache")

func get_object(uuid:UUID, type:LRUCache.ObjectType) -> PackedScene:
	if !await preload_object(uuid, type):
		push_warning("error in preload step, returning null")
		return null
	
	var response:Array[Variant] = await GlobalAPIHandler.make_request(HTTPClient.METHOD_GET, OBJECT_INFO_ENDPOINT % [uuid, type])
	var result:Array[Variant] = GlobalAPIHandler.handle_response(response[0], response[2], [200], ["root_name", "key", "iv"])
	
	var success:bool = result[0]
	var response_code:int = result[1]
	var error_code:String = result[2]
	var error_message:String = result[3]
	var response_values:Array[Variant] = result[4]
	
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
	return decrypt_and_load_object(file, uuid, response_values[0], response_values[1], response_values[2])

func preload_object(uuid:UUID, type:LRUCache.ObjectType) -> bool:
	var response:Array[Variant] = await GlobalAPIHandler.make_request(HTTPClient.METHOD_GET, OBJECT_INFO_ENDPOINT % [uuid, type])
	var result:Array[Variant] = GlobalAPIHandler.handle_response(response[0], response[2], [200], ["last_update_utc", "size_KB", "can_download"])
	
	var success:bool = result[0]
	var response_code:int = result[1]
	var error_code:String = result[2]
	var error_message:String = result[3]
	var response_values:Array[Variant] = result[4]
	
	if (!success) or !response_values[2]:
		push_warning("failed to aquire object data")
		if response_code != -1:
			push_error("server response: %s" % response_code)
		if error_code != "":
			push_error("error code: %s" % error_code)
		if error_message != "":
			push_error("error message: %s" % error_message)
		return false
	
	if cache.has(uuid, type):
		if cache.get_object(uuid, type).cache_time_utc >= response_values[0]:
			return true
		else:
			cache.pop_front()
	
	# cache value didnt exist or was stale so we download
	download_object(uuid, type)
	var item = LRUCache.Pack.new(uuid, type, response_values[0], response_values[1])
	cache.push_front(item)
	return true

func remove_all_expired() -> void:
	## todo
	push_error("not yet implemented")

func download_object(uuid:UUID, object_type:LRUCache.ObjectType) -> void:
	var url = OBJECT_DOWNLOAD_ENDPOINT % [uuid, object_type]
	var downloader:HTTPRequest = HTTPRequest.new()
	#FileAccess.open(file_path, FileAccess.WRITE).close()
	downloader.download_file = cache.object_file_path % [uuid, object_type]
	downloader.request(url, PackedStringArray([GlobalAccountHandler.session_token]))
	await downloader.request_completed

func decrypt_and_load_object(object:FileAccess, uuid:UUID, root_name:String, key:PackedByteArray, iv:PackedByteArray) -> PackedScene:
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
	return load("res://_loaded_content/%s/%s" % [uuid, root_name]) as PackedScene
