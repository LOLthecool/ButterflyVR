extends Button
class_name ObjectListing

const OBJECT_IMAGE_ENDPOINT:String = "api/v0/%s/%s/image"

signal object_selected(object:Dictionary[String, Variant])

var object:Dictionary[String, Variant]

func create(object:Dictionary[String, Variant], object_type:LRUCache.ObjectType) -> void:
	self.object = object
	text = object["name"]
	var response = await GlobalAPIHandler.make_request(HTTPClient.METHOD_GET, 
			OBJECT_IMAGE_ENDPOINT % [object_type, object["uuid"]], 
			PackedStringArray([GlobalAccountHandler.get_token_header()]))
	if response[0] == HTTPClient.RESPONSE_OK:
		var image:Image = Image.new()
		# if/elif statements here are just so we only get the side effects
		# from a single branch
		if image.load_png_from_buffer(response[2]) == OK:
			pass
		elif image.load_webp_from_buffer(response[2]) == OK:
			pass
		elif image.load_jpg_from_buffer(response[2]) == OK:
			pass
		else:
			return
		icon = ImageTexture.create_from_image(image)

func _pressed() -> void:
	object_selected.emit(object)
