extends Button

@export var token:LineEdit
@export var token_ui:ObjectVerifyTokenUI

func _ready() -> void:
	token_ui.token_generated.connect(on_generated)
	token_ui.reset.connect(on_reset)

func _pressed() -> void:
	DisplayServer.clipboard_set(token.text)

func on_generated(_token:UUID, _object_id:UUID, _creator:UUID) -> void:
	disabled = false

func on_reset() -> void:
	disabled = true
