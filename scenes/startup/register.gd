extends VBoxContainer
class_name RegisterMenu

@export var greeter:GreeterMenu
@export var sign_in:SignInMenu

@export var username:LineEdit
@export var email:LineEdit
@export var password:LineEdit
@export var password2:LineEdit

@export var checkbox:CheckBox
@export var checkbox2:CheckBox

@export var cancel_button:Button
@export var continue_button:Button


func _on_cancel_pressed() -> void:
	greeter.visible = true


func _on_sign_up_pressed() -> void:
	cancel_button.disabled = true
	continue_button.disabled = true
