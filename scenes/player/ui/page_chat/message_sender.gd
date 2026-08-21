extends Button

@export var message_box: LineEdit


func _pressed() -> void:
	if message_box.text.is_empty():
		return
	GlobalWorldHandler.current_world.chat_box_manager.send_message(message_box.text)
	message_box.clear()


func _on_line_edit_text_submitted(_new_text: String) -> void:
	_pressed()
