extends Button

@export var details_page:AvatarDetailsPage

func _pressed() -> void:
	details_page.visible = false
