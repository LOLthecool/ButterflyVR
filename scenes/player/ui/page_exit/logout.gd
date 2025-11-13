extends Button

@export var popup:Panel
@export var popup_text:Label
@export var confirm_button:Button
@export var cancel_button:Button
@export var popup_dialogue:String = "Are you sure?"

func _pressed() -> void:
	if popup.visible:
		return
	popup.visible = true
	popup_text.text = popup_dialogue
	confirm_button.pressed.connect(on_confirm)
	cancel_button.pressed.connect(on_cancel)

func on_confirm() -> void:
	clean_popup()
	GlobalWorldHandler.disconnect_from_world(false)
	GlobalAccountHandler.logout()
	get_tree().change_scene_to_packed(preload("res://scenes/startup/loading.tscn"))

func on_cancel() -> void:
	clean_popup()

func clean_popup() -> void:
	popup.visible = false
	popup_text.text = "UNNAMED"
	confirm_button.pressed.disconnect(on_confirm)
	cancel_button.pressed.disconnect(on_cancel)
