extends DownloadHandler
class_name ServerVariantDownloadHandler


func _init() -> void:
	object_download_endpoint = "/api/v0/%s/%s/server"
	cache_size = MEGABYTE * 200
	cache_file = "server_cache"
	object_file_path = "user://server_objects/%s.epck"

	backing_cache = LruCache.new_cache(cache_size, on_save, on_load, on_destroy)
	backing_cache.load()
