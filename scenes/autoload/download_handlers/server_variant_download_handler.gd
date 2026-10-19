extends DownloadHandler
class_name ServerVariantDownloadHandler


func _init() -> void:
	object_download_endpoint = "/api/v0/%s/%s/server"
	cache_size = MEGABYTE * 200
	cache_file = "server_cache"
	object_file_path = "user://server_objects/%s.epck"

	backing_cache = LruCache.new_cache(cache_size, on_save, on_load, on_destroy)
	backing_cache.load()


func decrypt_and_load_object(
	object: FileAccess,
	object_type: TypeHelper.ObjectType,
	uuid: String,
	key: PackedByteArray,
	iv: PackedByteArray,
) -> PackedScene:
	if !object:
		push_error("object %s was requested but did not exist in server cache" % uuid)
		backing_cache.pop(uuid)
		push_warning("falling back to base variant")
		return await GlobalDownloadHandler.get_object(UUID.from_String(uuid), object_type)
		return null

	var aes: AESContext = AESContext.new()
	aes.start(AESContext.MODE_CBC_DECRYPT, key, iv)

	object.seek(0)

	var decrypted: FileAccess = FileAccess.create_temp(FileAccess.READ_WRITE, "object", ".pck")

	while object.get_position() + (1024 * 1024) < object.get_length():
		decrypted.store_buffer(aes.update(object.get_buffer(1024 * 1024)))

	# encrypted files should always be a multiple of 16 bytes long
	decrypted.store_buffer(
		aes.update(object.get_buffer(object.get_length() - object.get_position()))
	)

	object.close()
	aes.finish()

	if decrypted.get_length() == 0:
		push_warning("got empty server variant from server")
		push_warning("falling back to base variant")
		return await GlobalDownloadHandler.get_object(UUID.from_String(uuid), object_type)

	decrypted.seek(decrypted.get_length() - 1)

	# trim pading bytes
	# padding bytes are 255 followed by 0s
	# there is always at least 1 padding byte (255)
	var zero_bytes: int = 0
	while decrypted.get_8() == 0:
		zero_bytes += 1
		decrypted.seek(decrypted.get_length() - (zero_bytes + 1))

	# remove all padding including 255
	decrypted.resize(decrypted.get_length() - (zero_bytes + 1))
	decrypted.flush()
	var decrypted_path: String = decrypted.get_path()

	var handle: FileAccess = FileAccess.create_temp(FileAccess.READ_WRITE, "object", ".pck", true)
	var new_object: String = handle.get_path()
	ZSTDCompressor.decompress_file_to_file(decrypted_path, new_object)
	decrypted.close()

	if !PCKChecker.is_pck_good(new_object, str(object_type), uuid):
		push_error("object %s failed verification, aborting load." % uuid)
		return null

	var load_thread: Thread = Thread.new()

	load_thread.start(
		func() -> bool:
			return ProjectSettings.load_resource_pack(new_object, true),
	)

	while load_thread.is_alive():
		await get_tree().physics_frame

	if !load_thread.wait_to_finish():
		push_error("failed to load object pck from %s" % new_object)
		return null

	load_thread.start(
		func() -> PackedScene:
			return ResourceLoader.load(
				"res://_loaded_content/%s/%s/root.tscn" % [object_type, uuid],
				"PackedScene",
				ResourceLoader.CACHE_MODE_IGNORE_DEEP,
			) as PackedScene,
	)

	while load_thread.is_alive():
		await get_tree().physics_frame

	return load_thread.wait_to_finish()
