extends Button
class_name ObjectListing

signal object_selected(object:Dictionary[String, Variant])

var object:Dictionary[String, Variant]

func create(object:Dictionary[String, Variant], object_type:LRUCache.ObjectType) -> void:
	self.object = object
	text = object["name"]
	icon = ImageTexture.create_from_image(await GlobalImageDownloadHandler.get_object(UUID.from_String(object["id"]), object_type))
	icon_alignment = HORIZONTAL_ALIGNMENT_CENTER
	vertical_icon_alignment = VERTICAL_ALIGNMENT_TOP
	expand_icon = true
	custom_minimum_size = Vector2(192, 192)

func _pressed() -> void:
	object_selected.emit(object)
