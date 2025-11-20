extends Button
class_name ObjectListing

signal object_selected(object:Dictionary[String, Variant])

var object:Dictionary[String, Variant]

func create(object:Dictionary[String, Variant], object_type:LRUCache.ObjectType) -> void:
	self.object = object
	text = object["name"]
	icon = ImageTexture.create_from_image(await GlobalImageDownloadHandler.get_object(object["uuid"], object_type))

func _pressed() -> void:
	object_selected.emit(object)
