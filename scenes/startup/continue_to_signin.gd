extends Button

@export var sign_in:SignInMenu

func _pressed() -> void:
	sign_in.visible = true
