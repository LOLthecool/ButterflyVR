extends Button
class_name ObjectListing

signal object_selected(object: Dictionary[String, Variant])

var object: Dictionary[String, Variant]
var loaded_image: bool = false


func create(object: Dictionary[String, Variant], object_type: TypeHelper.ObjectType) -> void:
	self.object = object
	text = object["name"]
	icon = ImageTexture.new()
	icon_alignment = HORIZONTAL_ALIGNMENT_CENTER
	vertical_icon_alignment = VERTICAL_ALIGNMENT_TOP
	expand_icon = true
	custom_minimum_size = Vector2(192, 192)
	var notifier: VisibleOnScreenNotifier2D = VisibleOnScreenNotifier2D.new()
	notifier.screen_entered.connect(get_image.bind(object_type))
	add_child(notifier)


func get_image(object_type: TypeHelper.ObjectType) -> void:
	@warning_ignore("unsafe_cast")
	icon = ImageTexture.create_from_image(
		await GlobalImageDownloadHandler.get_object(
			UUID.from_String(object["id"] as String),
			object_type,
		)
	)


func _pressed() -> void:
	object_selected.emit(object)
