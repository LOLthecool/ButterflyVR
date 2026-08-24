extends Node
class_name DownloadHandler

const OBJECT_INFO_ENDPOINT: String = "/api/v0/%s/%s"
const OBJECT_DOWNLOAD_ENDPOINT: String = "/api/v0/%s/%s/epck"
const MEGABYTE: int = 1024 * 1024
const GIGABYTE: int = MEGABYTE * 1024
const CACHE_SIZE: int = GIGABYTE * 10
const CACHE_FILE: String = "object_cache"
const OBJECT_FILE_PATH: String = "user://objects/%s.epck"

var backing_cache: LruCache
var cache_lock: Mutex = Mutex.new()


func on_save(cached_objects: Dictionary) -> void:
	GlobalPersistanceHandler.clear_catagory(CACHE_FILE, "values", false)
	for uuid: String in cached_objects.keys():
		GlobalPersistanceHandler.register_value(
			CACHE_FILE,
			"values",
			uuid,
			cached_objects[uuid],
			false,
		)
	GlobalPersistanceHandler.flush_file(CACHE_FILE)


func on_load() -> Dictionary:
	return GlobalPersistanceHandler.get_catagory(CACHE_FILE, "values")


func on_destroy(uuid: String) -> void:
	DirAccess.remove_absolute(OBJECT_FILE_PATH % uuid)


func _init() -> void:
	backing_cache = LruCache.new_cache(CACHE_SIZE, on_save, on_load, on_destroy)
	backing_cache.load()


func get_object(uuid: UUID, type: TypeHelper.ObjectType) -> PackedScene:
	var id: String = uuid.to_string()
	if !await preload_object(id, type):
		push_warning("error in preload step, returning null")
		return null

	var object_type_string: String = "UNNAMED"
	match type:
		TypeHelper.ObjectType.world:
			object_type_string = "World"
		TypeHelper.ObjectType.avatar:
			object_type_string = "Avatar"

	# todo: make request once and then pass in values to preload
	var response: Array[Variant] = await GlobalAPIHandler.make_request(
		HTTPClient.METHOD_GET,
		OBJECT_INFO_ENDPOINT % [object_type_string, uuid],
		PackedStringArray([GlobalAccountHandler.get_token_header()]),
	)
	@warning_ignore("unsafe_call_argument") var result: Array[Variant] = GlobalAPIHandler.handle_response(
		response[0],
		response[2],
		[200],
		["encryption_key", "encryption_iv"],
	)

	var success: bool = result[0]
	var response_code: int = result[1]
	var error_code: String = result[2]
	var error_message: String = result[3]
	var response_values: Dictionary[String, Variant] = result[4]

	if (!success):
		push_warning("failed to aquire object data")
		if response_code != -1:
			push_error("server response: %s" % response_code)
		if error_code != "":
			push_error("error code: %s" % error_code)
		if error_message != "":
			push_error("error message: %s" % error_message)
		return null

	await MiscHelpers.await_lock_mutex(cache_lock)

	var file: FileAccess = FileAccess.open(OBJECT_FILE_PATH % [id], FileAccess.READ)
	@warning_ignore("unsafe_cast") var object: PackedScene = await decrypt_and_load_object(
		file,
		type,
		id,
		response_values["encryption_key"] as PackedByteArray,
		response_values["encryption_iv"] as PackedByteArray,
	)

	cache_lock.unlock()

	return object


func preload_object(id: String, type: TypeHelper.ObjectType) -> bool:
	var object_type_string: String = "UNNAMED"

	match type:
		TypeHelper.ObjectType.world:
			object_type_string = "World"
		TypeHelper.ObjectType.avatar:
			object_type_string = "Avatar"

	var response: Array[Variant] = await GlobalAPIHandler.make_request(
		HTTPClient.METHOD_GET,
		OBJECT_INFO_ENDPOINT % [object_type_string, id],
		PackedStringArray([GlobalAccountHandler.get_token_header()]),
	)
	@warning_ignore("unsafe_call_argument") var result: Array[Variant] = GlobalAPIHandler.handle_response(
		response[0],
		response[2],
		[200],
		["updated_at", "object_size"],
	)

	var success: bool = result[0]
	var response_code: int = result[1]
	var error_code: String = result[2]
	var error_message: String = result[3]
	var response_values: Dictionary[String, Variant] = result[4]

	if (!success):
		push_warning("failed to aquire object data")
		if response_code != -1:
			push_error("server response: %s" % response_code)
		if error_code != "":
			push_error("error code: %s" % error_code)
		if error_message != "":
			push_error("error message: %s" % error_message)
		return false

	await MiscHelpers.await_lock_mutex(cache_lock)

	var object: Dictionary[String, int] = { }
	object.assign(backing_cache.get(id))
	if !object.is_empty():
		if (!FileAccess.file_exists(OBJECT_FILE_PATH % [id])) or \
				FileAccess.get_size(OBJECT_FILE_PATH % [id]) < 1:
			push_error("cached file did not exist for object: %s" % id)
			backing_cache.pop(id)
		else:
			if object.cache_time_utc >= response_values["updated_at"]:
				backing_cache.save()
				return true
			else:
				backing_cache.pop(id)

	# cache value didnt exist or was stale so we download
	await download_object(id, type)
	@warning_ignore("unsafe_cast")
	backing_cache.push_front(
		id,
		response_values["updated_at"] as int,
		int(ceilf(response_values["object_size"] as float / 1024)),
	)
	backing_cache.save()

	cache_lock.unlock()

	return true


func download_object(uuid: String, object_type: TypeHelper.ObjectType) -> void:
	var object_type_string: String = "UNNAMED"

	match object_type:
		TypeHelper.ObjectType.world:
			object_type_string = "World"
		TypeHelper.ObjectType.avatar:
			object_type_string = "Avatar"

	var url: String = OBJECT_DOWNLOAD_ENDPOINT % [object_type_string, uuid]

	var downloader: HTTPRequest = HTTPRequest.new()
	add_child(downloader)

	downloader.download_file = OBJECT_FILE_PATH % [uuid]

	if !DirAccess.dir_exists_absolute(OBJECT_FILE_PATH.trim_suffix("%s.epck")):
		DirAccess.make_dir_recursive_absolute(OBJECT_FILE_PATH.trim_suffix("%s.epck"))

	FileAccess.open(downloader.download_file, FileAccess.WRITE).close()

	if GlobalAPIHandler.target_port == 443:
		downloader.request(
			"https://" + GlobalAPIHandler.target_host + ":"
			+ str(GlobalAPIHandler.target_port) + url,
			PackedStringArray([GlobalAccountHandler.get_token_header()]),
		)
	else:
		downloader.request(
			"http://" + GlobalAPIHandler.target_host + ":" + str(GlobalAPIHandler.target_port) + url,
			PackedStringArray([GlobalAccountHandler.get_token_header()]),
		)

	await downloader.request_completed

	downloader.queue_free()


func decrypt_and_load_object(
	object: FileAccess,
	object_type: TypeHelper.ObjectType,
	uuid: String,
	key: PackedByteArray,
	iv: PackedByteArray,
) -> PackedScene:
	if !object:
		push_warning("object %s did not exist in cache" % uuid)
		backing_cache.pop(uuid)
		return null

	var aes: AESContext = AESContext.new()
	aes.start(AESContext.MODE_CBC_DECRYPT, key, iv)

	object.seek(0)

	var decrypted_buffer: PackedByteArray = PackedByteArray()

	while object.get_position() + (1024 * 1024) < object.get_length():
		decrypted_buffer += aes.update(object.get_buffer(1024 * 1024))

	# encrypted files should always be a multiple of 16 bytes long
	decrypted_buffer += aes.update(object.get_buffer(object.get_length() - object.get_position()))

	object.close()
	aes.finish()

	if decrypted_buffer.size() == 0:
		push_warning("got empty object from server")
		return null

	# trim pading bytes
	# padding bytes are 255 followed by 0s
	# there is always at least 1 padding byte (255)
	var zero_bytes: int = 0
	while decrypted_buffer[(decrypted_buffer.size() - zero_bytes) - 1] == 0:
		zero_bytes += 1

	if decrypted_buffer[(decrypted_buffer.size() - zero_bytes) - 1] != 255:
		push_error("object was not correctly padded, attempting to continue decoding")
		# if this is causing issues its probably safe to remove and just reject bad padding outright
		zero_bytes -= 1 # assumes the last byte of the object isnt 0, but this shouldnt happen anyways

	# remove all padding including 255
	decrypted_buffer.resize(decrypted_buffer.size() - (zero_bytes + 1))
	var decrypted: FileAccess = FileAccess.create_temp(FileAccess.READ_WRITE, "object", ".pck")
	decrypted.store_buffer(decrypted_buffer)
	decrypted.flush()
	var decrypted_path: String = decrypted.get_path()

	# todo: ram backed tmp files to avoid storing decrypted pcks
	var handle: FileAccess = FileAccess.create_temp(FileAccess.READ_WRITE, "object", ".pck", true)
	var new_object: String = handle.get_path()
	ZSTDCompressor.decompress_file_to_file(decrypted_path, new_object)
	decrypted.close()
	
	if !PCKChecker.is_pck_good(new_object, str(object_type), uuid):
		push_error("object %s failed verification, aborting load." % uuid)
		return null
	
	var load_thread:Thread = Thread.new()
	
	# PCKChecker checks all files are within _loaded_content/object_type/uuid 
	# so overwriting is safe. 
	load_thread.start(func() -> bool:
		return ProjectSettings.load_resource_pack(new_object))
	
	while load_thread.is_alive():
		await get_tree().physics_frame
	
	if !load_thread.wait_to_finish():
		push_error("failed to load object pck from %s" % new_object)
		return null
	
	load_thread.start(func() -> PackedScene:
		return ResourceLoader.load(
		"res://_loaded_content/%s/%s/root.tscn" % [object_type, uuid],
		"PackedScene",
		ResourceLoader.CACHE_MODE_IGNORE_DEEP,
	) as PackedScene)
	
	while load_thread.is_alive():
		await get_tree().physics_frame
	
	return load_thread.wait_to_finish()


func _physics_process(_delta: float) -> void:
	backing_cache.process_destroy_queue()
