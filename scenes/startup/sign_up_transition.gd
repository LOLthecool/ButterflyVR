extends VBoxContainer
class_name RegisterTransitionMenu

@export var sign_in:SignInMenu

func _on_button_pressed() -> void:
	sign_in.visible = true
