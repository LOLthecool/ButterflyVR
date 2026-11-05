extends VBoxContainer
class_name GreeterMenu

@export var sign_up:RegisterMenu
@export var sign_in:SignInMenu

func _on_sign_up_pressed() -> void:
	sign_up.visible = true


func _on_sign_in_pressed() -> void:
	sign_in.visible = true


func _on_exit_pressed() -> void:
	get_tree().quit()
