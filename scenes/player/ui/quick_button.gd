extends Button

@export var page:PackedScene
@export var page_handler:PageHandler

func _pressed() -> void:
	print("pressed")
	page_handler.change_current_page(page.instantiate() as Page)
